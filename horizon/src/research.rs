// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeSet;

use mod_api::{
    Compound, ConfigFileError, ConfigPath, GameMode, Host, HostTransaction, Identifier, PlayerRef,
    ServerHost, Tag,
};

use crate::{guide, guide::research_name, recipes::{self, Research}};

const PERSISTED: &str = "PlayerPersisted";

pub(crate) fn can_use(host: &dyn Host, player: PlayerRef, research: &Research) -> bool {
    player.game_mode(host) == Some(GameMode::Creative)
        || player
            .persistent_data(host)
            .as_ref()
            .is_some_and(|data| unlocked(data, research))
}

pub(crate) fn unlocked(data: &Compound, research: &Research) -> bool {
    match data.get(&PERSISTED.into()) {
        Some(Tag::Compound(saved)) => unlocked_persisted(saved, research),
        _ => false,
    }
}

pub(crate) fn unlocked_persisted(data: &Compound, research: &Research) -> bool {
    let key = format!("slimefun:{}", research.id);
    matches!(data.get(&key.as_str().into()), Some(Tag::Byte(1)))
}

pub(crate) fn unlock(host: &mut dyn ServerHost, player: PlayerRef, research: &Research) -> String {
    let free = player.game_mode(host) == Some(GameMode::Creative);
    match begin(host, player, research, free, false) {
        Ok(()) => format!("已解锁“{}”。", research_name(research.id)),
        Err(message) => message,
    }
}

const PENDING: &str = "slimefun:pending_research";
const SYNC_GUIDE_ISSUED: &str = "slimefun:sync_guide_issued";
pub(crate) const SKIP_LEGACY_IMPORT: &str = "slimefun:skip_legacy_research_import";
const UPDATE_ERROR: &str = "远古工艺无法更新你的研究数据。";

fn saved_data(data: &mut Compound) -> Option<&mut Compound> {
    let Tag::Compound(saved) = data
        .entry(PERSISTED.into())
        .or_insert_with(|| Tag::Compound(Compound::new()))
    else {
        return None;
    };
    Some(saved)
}

fn mark_learned(data: &mut Compound, research: &Research) {
    data.insert(
        format!("slimefun:{}", research.id).as_str().into(),
        Tag::Byte(1),
    );
}

#[derive(Clone, Copy)]
pub(crate) enum BulkChange {
    UnlockAll,
    Clear,
}

pub(crate) fn bulk_change(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    change: BulkChange,
) -> Result<usize, &'static str> {
    let mut data = player.persistent_data(host).ok_or(UPDATE_ERROR)?;
    let changed = apply_bulk_change(&mut data, change)?;
    player.set_persistent_data(host, data).map_err(|_| UPDATE_ERROR)?;
    Ok(changed)
}

fn apply_bulk_change(data: &mut Compound, change: BulkChange) -> Result<usize, &'static str> {
    let saved = saved_data(data).ok_or(UPDATE_ERROR)?;
    saved.remove(&PENDING.into());
    let current = recipes::researches()
        .chain(guide::research_topics())
        .filter(|topic| !topic.id.is_empty())
        .map(|topic| topic.id);
    let mut changed = 0;
    match change {
        BulkChange::UnlockAll => {
            for id in current {
                let key = format!("slimefun:{id}");
                changed += usize::from(saved.insert(key.as_str().into(), Tag::Byte(1)) != Some(Tag::Byte(1)));
            }
        }
        BulkChange::Clear => {
            let legacy = include_str!("knowledge_researches.tsv")
                .lines()
                .filter_map(|row| row.split_once('\t').map(|(id, _)| id));
            for id in current.chain(legacy) {
                let key = format!("slimefun:{id}");
                changed += usize::from(saved.remove(&key.as_str().into()) == Some(Tag::Byte(1)));
            }
            saved.insert(SKIP_LEGACY_IMPORT.into(), Tag::Byte(1));
        }
    }
    Ok(changed)
}

fn store_payment(data: &mut Compound, research: &Research, delayed: bool) -> Result<(), String> {
    if unlocked(data, research) {
        return Err(format!("已解锁“{}”。", research_name(research.id)));
    }
    let saved = saved_data(data).ok_or(UPDATE_ERROR)?;
    if saved.contains_key(&PENDING.into()) {
        return Err("请先完成当前研究。".into());
    }
    if delayed {
        saved.insert(
            PENDING.into(),
            Tag::compound([
                ("id", Tag::String(research.id.into())),
                ("step", Tag::Byte(0)),
            ]),
        );
    } else {
        mark_learned(saved, research);
    }
    Ok(())
}

/// Persist the purchased topic together with its experience cost. Delayed topics
/// remain locked until the last step replaces the record with the learned flag.
pub(crate) fn begin(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    research: &Research,
    free: bool,
    delayed: bool,
) -> Result<(), String> {
    let mut data = player
        .persistent_data(host)
        .ok_or("远古工艺无法读取你的研究数据。")?;
    store_payment(&mut data, research, delayed)?;
    let experience = if free {
        None
    } else {
        let experience = player
            .experience(host)
            .ok_or("远古工艺无法读取你的经验。")?;
        if experience.level < research.cost {
            return Err(format!(
                "解锁“{}”需要 {} 级经验。",
                research_name(research.id), research.cost
            ));
        }
        Some(experience.level - research.cost)
    };
    host.transaction(|host, _| {
        player
            .set_persistent_data(host, data)
            .map_err(|_| UPDATE_ERROR)?;
        if let Some(level) = experience
            && !player.set_experience_levels(host, level)
        {
            return Err(UPDATE_ERROR);
        }
        Ok(())
    })
    .map_err(str::to_owned)
}

pub(crate) fn pending(data: &Compound) -> Option<(String, usize)> {
    let saved = data.get(&PERSISTED.into())?;
    let record = saved.get(PENDING)?;
    let (Tag::String(id), Tag::Byte(step)) = (record.get("id")?, record.get("step")?) else {
        return None;
    };
    Some((id.to_string_lossy(), (*step).try_into().ok()?))
}

/// Return false when the pending record is gone or has already advanced. A host
/// write failure is retryable and keeps the last durable step.
pub(crate) fn advance(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    research: &Research,
    step: usize,
    next: Option<i8>,
) -> Result<bool, ()> {
    let mut data = player.persistent_data(host).ok_or(())?;
    if !advance_saved(&mut data, research, step, next) {
        return Ok(false);
    }
    player.set_persistent_data(host, data).map_err(|_| ())?;
    Ok(true)
}

fn advance_saved(data: &mut Compound, research: &Research, step: usize, next: Option<i8>) -> bool {
    let Some((id, saved_step)) = pending(data) else {
        return false;
    };
    if id != research.id || saved_step != step {
        return false;
    }
    let Some(saved) = saved_data(data) else {
        return false;
    };
    if let Some(next) = next {
        if let Some(Tag::Compound(record)) = saved.get_mut(&PENDING.into()) {
            record.insert("step".into(), Tag::Byte(next));
        }
    } else {
        saved.remove(&PENDING.into());
        mark_learned(saved, research);
    }
    true
}

/// Merge a settled donor's recognized research flags without sharing their
/// settings or pending study. A matching paid study is now complete.
pub(crate) fn grant_shared(
    data: &mut Compound,
    known: &BTreeSet<String>,
) -> Result<Vec<String>, &'static str> {
    let active = pending(data).map(|(id, _)| format!("slimefun:{id}"));
    let saved = saved_data(data).ok_or(UPDATE_ERROR)?;
    let mut learned = Vec::new();
    for key in known {
        if saved.get(&key.as_str().into()) != Some(&Tag::Byte(1)) {
            saved.insert(key.as_str().into(), Tag::Byte(1));
            learned.push(key.to_owned());
            if active.as_ref() == Some(key) {
                saved.remove(&PENDING.into());
            }
        }
    }
    Ok(learned)
}

fn read_legacy(host: &dyn ServerHost, uuid: &str) -> Result<Option<String>, &'static str> {
    let path = ConfigPath::new(&format!("research-import/{uuid}.txt"))
        .expect("a hexadecimal UUID is a valid import file name");
    match host.world_data_files().read(&path) {
        Ok(bytes) if bytes.len() <= 64 * 1024 => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| "The saved Paper research archive is invalid."),
        Err(ConfigFileError::NotFound) => Ok(None),
        _ => Err("The saved Paper research archive is unreadable."),
    }
}

pub(crate) fn legacy_research_flags(
    host: &dyn ServerHost,
    uuid: u128,
) -> Result<Option<Vec<String>>, &'static str> {
    let owner = format!("{uuid:032x}");
    let mut flags = read_legacy(host, &owner)?
        .map(|text| {
            legacy_research_ids(&owner, &text)
                .map(|ids| ids.into_iter().map(str::to_owned).collect())
                .ok_or("The saved Paper research archive is invalid.")
        })
        .transpose()?;
    let synchronized =
        crate::pm_profile::with_archive(host, uuid, |profile| Ok(profile.slimefun_researches()))
            .map_err(|_| "The saved Slimefun sync archive is invalid.")?;
    if let Some(synchronized) = synchronized {
        flags.get_or_insert_with(Vec::new).extend(synchronized);
    }
    Ok(flags)
}

/// Import canonical research IDs prepared from a Paper legacy profile when its UUID joins.
pub(crate) fn import_legacy(host: &mut dyn ServerHost, player: PlayerRef) {
    let uuid = format!("{:032x}", player.uuid());
    let Some(data) = player.persistent_data(host) else {
        return;
    };
    if data.get(&PERSISTED.into()).and_then(|saved| saved.get(SKIP_LEGACY_IMPORT)) == Some(&Tag::Byte(1)) {
        return;
    }
    let Ok(Some(text)) = read_legacy(host, &uuid) else {
        return;
    };
    if let Some(merged) = merge_import(data, &uuid, &text) {
        let _ = player.set_persistent_data(host, merged);
    }
}

pub(crate) fn import_synchronized_profile(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(data) = player.persistent_data(host) else {
        return;
    };
    let profile = crate::pm_profile::with_archive(host, player.uuid(), |profile| {
        Ok((profile.guide_issued, profile.slimefun_researches()))
    });
    let (guide_issued, researches) = match profile {
        Ok(Some(profile)) => profile,
        Ok(None) => return,
        Err(error) => {
            let _ = player.tell(host, mod_api::Text::literal(error));
            return;
        }
    };
    if let Some(data) = merge_synchronized_profile(data, guide_issued, &researches) {
        let _ = player.set_persistent_data(host, data);
    }
}

fn merge_synchronized_profile(
    mut data: Compound,
    guide_issued: bool,
    researches: &BTreeSet<String>,
) -> Option<Compound> {
    let saved = saved_data(&mut data)?;
    let skip = saved.get(&SKIP_LEGACY_IMPORT.into()) == Some(&Tag::Byte(1));
    let mut changed = false;
    if guide_issued {
        changed = saved.insert(SYNC_GUIDE_ISSUED.into(), Tag::Byte(1)) != Some(Tag::Byte(1));
    }
    if !skip {
        changed |= grant_shared(&mut data, researches).is_ok_and(|learned| !learned.is_empty());
    }
    changed.then_some(data)
}

pub(crate) fn sync_guide_already_issued(host: &dyn Host, player: PlayerRef) -> bool {
    player.persistent_data(host).is_some_and(|data| {
        data.get(&PERSISTED.into())
            .and_then(|saved| saved.get(SYNC_GUIDE_ISSUED))
            == Some(&Tag::Byte(1))
    })
}

fn legacy_research_ids<'a>(uuid: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let mut lines = text.lines();
    if lines.next()? != "slimefun-research-import-v1" || lines.next()? != uuid {
        return None;
    }
    lines
        .map(|line| {
            let id = Identifier::parse(line).ok()?;
            (id.namespace() == "slimefun" && line.starts_with("slimefun:")).then_some(line)
        })
        .collect()
}

fn merge_import(mut data: Compound, uuid: &str, text: &str) -> Option<Compound> {
    let keys = legacy_research_ids(uuid, text)?;
    let saved = saved_data(&mut data)?;
    let mut changed = false;
    for line in keys {
        let old = saved.insert(line.into(), Tag::Byte(1));
        changed |= old != Some(Tag::Byte(1));
    }
    changed.then_some(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::LUMPS;

    const ID: &str = "00000000000000000000000000000001";

    #[test]
    fn synchronized_research_does_not_regrant_manually_cleared_research_or_a_guide() {
        let flags = BTreeSet::from(["slimefun:lumps".to_owned()]);
        let imported = merge_synchronized_profile(Compound::new(), true, &flags).unwrap();
        assert!(unlocked(&imported, &LUMPS));
        assert!(merge_synchronized_profile(imported.clone(), true, &flags).is_none());

        let mut cleared = imported;
        apply_bulk_change(&mut cleared, BulkChange::Clear).unwrap();
        assert!(!unlocked(&cleared, &LUMPS));
        assert_eq!(
            cleared
                .get(&PERSISTED.into())
                .unwrap()
                .get(SYNC_GUIDE_ISSUED),
            Some(&Tag::Byte(1))
        );
        assert!(merge_synchronized_profile(cleared, true, &flags).is_none());
    }

    #[test]
    fn bulk_research_changes_preserve_preferences_and_do_not_resume_a_cancelled_study() {
        let mut data = Compound::new();
        store_payment(&mut data, &LUMPS, true).unwrap();
        let unrelated = "slimefun:research_fireworks";
        saved_data(&mut data).unwrap().insert(unrelated.into(), Tag::Byte(1));
        assert!(apply_bulk_change(&mut data, BulkChange::UnlockAll).unwrap() > 200);
        assert_eq!(apply_bulk_change(&mut data, BulkChange::UnlockAll), Ok(0));
        assert!(unlocked(&data, &LUMPS));
        assert!(pending(&data).is_none());
        assert!(apply_bulk_change(&mut data, BulkChange::Clear).unwrap() > 200);
        assert!(!unlocked(&data, &LUMPS));
        assert!(pending(&data).is_none());
        let saved = data.get(&PERSISTED.into()).unwrap();
        assert_eq!(saved.get(unrelated), Some(&Tag::Byte(1)));
        assert_eq!(saved.get(SKIP_LEGACY_IMPORT), Some(&Tag::Byte(1)));
    }
    const IMPORT: &str = concat!(
        "slimefun-research-import-v1\n",
        "00000000000000000000000000000001\n",
        "slimefun:lumps\n"
    );

    #[test]
    fn legacy_import_merges_without_erasing_other_data() {
        let old = Compound::from([(
            PERSISTED.into(),
            Tag::compound([("other:flag", Tag::Byte(1))]),
        )]);
        let merged = merge_import(old, ID, IMPORT).unwrap();
        assert!(unlocked(&merged, &LUMPS));
        assert_eq!(
            merged.get(&PERSISTED.into()).unwrap().get("other:flag"),
            Some(&Tag::Byte(1))
        );
        assert!(merge_import(merged, ID, IMPORT).is_none());
    }

    #[test]
    fn paid_research_remains_locked_at_each_saved_step_until_completion() {
        let mut data = Compound::new();
        store_payment(&mut data, &LUMPS, true).unwrap();
        assert!(!unlocked(&data, &LUMPS));
        assert_eq!(pending(&data), Some((LUMPS.id.into(), 0)));
        let original = data.clone();
        assert!(store_payment(&mut data, &LUMPS, false).is_err());
        assert!(!advance_saved(&mut data, &LUMPS, 1, Some(2)));
        assert_eq!(data, original);
        for step in 0..5 {
            assert!(advance_saved(
                &mut data,
                &LUMPS,
                step,
                Some((step + 1) as i8)
            ));
            assert!(!unlocked(&data, &LUMPS));
            assert_eq!(pending(&data), Some((LUMPS.id.into(), step + 1)));
        }
        assert!(advance_saved(&mut data, &LUMPS, 5, None));
        assert!(pending(&data).is_none());
        assert!(unlocked(&data, &LUMPS));
        assert!(!advance_saved(&mut data, &LUMPS, 5, None));
    }

    #[test]
    fn shared_flags_merge_without_overwriting_other_data_or_finishing_an_unrelated_study() {
        let mut data = Compound::from([(
            PERSISTED.into(),
            Tag::compound([("slimefun:research_fireworks", Tag::Byte(0))]),
        )]);
        store_payment(&mut data, &LUMPS, true).unwrap();
        let brewing = BTreeSet::from(["slimefun:auto_brewer".to_owned()]);
        assert_eq!(
            grant_shared(&mut data, &brewing).unwrap(),
            ["slimefun:auto_brewer"]
        );
        assert_eq!(pending(&data), Some((LUMPS.id.into(), 0)));
        assert!(grant_shared(&mut data, &brewing).unwrap().is_empty());
        let matching = BTreeSet::from(["slimefun:lumps".to_owned()]);
        assert_eq!(
            grant_shared(&mut data, &matching).unwrap(),
            ["slimefun:lumps"]
        );
        assert!(unlocked(&data, &LUMPS));
        assert!(pending(&data).is_none());
        assert_eq!(
            data.get(&PERSISTED.into())
                .unwrap()
                .get("slimefun:research_fireworks"),
            Some(&Tag::Byte(0))
        );
    }

    #[test]
    fn legacy_import_rejects_the_wrong_uuid_or_foreign_keys() {
        assert!(merge_import(Compound::new(), "other-player", IMPORT).is_none());
        assert!(merge_import(Compound::new(), ID, &format!("{IMPORT}other:flag\n")).is_none());
    }
}
