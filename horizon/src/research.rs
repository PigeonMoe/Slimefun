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
    let Some(mut data) = player.persistent_data(host) else {
        return "Slimefun could not read your research data.".into();
    };
    if unlocked(&data, research) {
        return format!("Already researched {}.", research.name);
    }
    let creative = player.game_mode(host) == Some(GameMode::Creative);
    let Some(experience) = player.experience(host) else {
        return "Slimefun could not read your experience.".into();
    };
    if !creative && experience.level < research.cost {
        return format!(
            "{} costs {} experience levels.",
            research.name, research.cost
        );
    }

    let saved = data
        .entry(PERSISTED.into())
        .or_insert_with(|| Tag::Compound(Compound::new()));
    let Tag::Compound(saved) = saved else {
        return "Slimefun could not update your research data.".into();
    };
    saved.insert(
        format!("slimefun:{}", research.id).as_str().into(),
        Tag::Byte(1),
    );

    let result = host.transaction(|host, _| {
        player.set_persistent_data(host, data).map_err(|_| ())?;
        if !creative && !player.set_experience_levels(host, experience.level - research.cost) {
            return Err(());
        }
        Ok(())
    });
    match result {
        Ok(()) => format!("Researched {}.", research.name),
        Err(()) => "Slimefun could not update your research data.".into(),
    }
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
    fn legacy_import_rejects_the_wrong_uuid_or_foreign_keys() {
        assert!(merge_import(Compound::new(), "other-player", IMPORT).is_none());
        assert!(merge_import(Compound::new(), ID, &format!("{IMPORT}other:flag\n")).is_none());
    }
}
