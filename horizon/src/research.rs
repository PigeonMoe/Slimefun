// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, GameMode, Host, HostTransaction, PlayerRef, ServerHost, Tag};

use crate::recipes::Research;

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
        Ok(()) => format!("Researched {}.", research.name),
        Err(message) => message,
    }
}

const PENDING: &str = "slimefun:pending_research";
const UPDATE_ERROR: &str = "Slimefun could not update your research data.";

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

fn store_payment(data: &mut Compound, research: &Research, delayed: bool) -> Result<(), String> {
    if unlocked(data, research) {
        return Err(format!("Already researched {}.", research.name));
    }
    let saved = saved_data(data).ok_or(UPDATE_ERROR)?;
    if saved.contains_key(&PENDING.into()) {
        return Err("Finish your current research first.".into());
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
        .ok_or("Slimefun could not read your research data.")?;
    store_payment(&mut data, research, delayed)?;
    let experience = if free {
        None
    } else {
        let experience = player
            .experience(host)
            .ok_or("Slimefun could not read your experience.")?;
        if experience.level < research.cost {
            return Err(format!(
                "{} costs {} experience levels.",
                research.name, research.cost
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

/// Import canonical research IDs prepared from a Paper legacy profile when its UUID joins.
pub(crate) fn import_legacy(host: &mut dyn ServerHost, player: PlayerRef) {
    let uuid = format!("{:032x}", player.uuid());
    let path = mod_api::ConfigPath::new(&format!("research-import/{uuid}.txt"))
        .expect("a hexadecimal UUID is a valid import file name");
    let Ok(bytes) = host.world_data_files().read(&path) else {
        return;
    };
    if bytes.len() > 64 * 1024 {
        return;
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return;
    };
    let Some(data) = player.persistent_data(host) else {
        return;
    };
    if let Some(merged) = merge_import(data, &uuid, text) {
        let _ = player.set_persistent_data(host, merged);
    }
}

fn merge_import(mut data: Compound, uuid: &str, text: &str) -> Option<Compound> {
    let mut lines = text.lines();
    if lines.next()? != "slimefun-research-import-v1" || lines.next()? != uuid {
        return None;
    }
    let saved = data
        .entry(PERSISTED.into())
        .or_insert_with(|| Tag::Compound(Compound::new()));
    let Tag::Compound(saved) = saved else {
        return None;
    };
    let mut changed = false;
    for line in lines {
        let id = mod_api::Identifier::parse(line).ok()?;
        if id.namespace() != "slimefun" || !line.starts_with("slimefun:") {
            return None;
        }
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
    fn legacy_import_rejects_the_wrong_uuid_or_foreign_keys() {
        assert!(merge_import(Compound::new(), "other-player", IMPORT).is_none());
        assert!(merge_import(Compound::new(), ID, &format!("{IMPORT}other:flag\n")).is_none());
    }
}
