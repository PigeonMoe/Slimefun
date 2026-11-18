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

fn unlocked(data: &Compound, research: &Research) -> bool {
    let key = format!("slimefun:{}", research.id);
    matches!(
        data.get(&PERSISTED.into())
            .and_then(|saved| saved.get(&key)),
        Some(Tag::Byte(1))
    )
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
