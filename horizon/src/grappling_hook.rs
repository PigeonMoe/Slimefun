// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    ActiveEffect, ArrowContactPostEvent, ArrowPickup, Compound, EntityJoinLevelEvent, EntityRef,
    GameMode, Hand, Host, ItemStack, LevelRef, PlayerRef, PlayerTeleportStatus,
    ProjectileImpactEvent, ServerHost, Tag,
};

use crate::{
    hologram_projector::colored_lore,
    item_clicks,
    items::{STEEL_INGOT, item},
    jetpack::{CHAIN, GRAPPLING_RESEARCH},
    recipes::{CraftingRecipe, Stack},
    research,
};

item!(HOOK, "flint", "Hook", "aqua", None, None);
item!(GRAPPLING_HOOK, "lead", "Grappling Hook", "gold", None, None);

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            None,
            Some(STEEL_INGOT),
            None,
            Some(STEEL_INGOT),
            None,
            Some(STEEL_INGOT),
            None,
            None,
            None,
        ],
        output: Stack {
            kind: HOOK,
            count: 1,
        },
        research: &GRAPPLING_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            None,
            Some(HOOK),
            None,
            Some(CHAIN),
            None,
            Some(CHAIN),
            None,
            None,
        ],
        output: Stack {
            kind: GRAPPLING_HOOK,
            count: 1,
        },
        research: &GRAPPLING_RESEARCH,
    },
];

pub(crate) const DEFAULT_CONFIG: &str =
    "\n[grappling-hook]\nconsume-on-use = true\ndespawn-seconds = 60\n";
const ARROW_TAG: &str = "slimefun.grappling_hook_arrow";
const BAT_TAG: &str = "slimefun.grappling_hook_bat";

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id == GRAPPLING_HOOK.id() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(vec![
                Tag::String("".into()),
                colored_lore("&eRight Click&7 to use"),
            ]),
        );
    }
}

#[derive(Clone, Copy)]
struct Config {
    consume: bool,
    lifespan: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            consume: true,
            lifespan: 60,
        }
    }
}

#[derive(Clone, Copy)]
struct Tether {
    arrow: EntityRef,
    bat: EntityRef,
    owner: EntityRef,
    spent: bool,
    check_hit_at: Option<i64>,
}

#[derive(Clone, Copy)]
struct Shot {
    level: LevelRef,
    until: i64,
    tether: Option<Tether>,
}

#[derive(Default)]
pub(crate) struct GrapplingHooks {
    config: Mutex<Config>,
    shots: Mutex<BTreeMap<u128, Shot>>,
}

impl GrapplingHooks {
    pub(crate) fn configure(&self, section: Option<&toml::Value>) {
        let mut config = Config::default();
        if let Some(section) = section {
            config.consume = section
                .get("consume-on-use")
                .and_then(toml::Value::as_bool)
                .unwrap_or(config.consume);
            config.lifespan = section
                .get("despawn-seconds")
                .and_then(toml::Value::as_integer)
                .unwrap_or(config.lifespan)
                .clamp(0, i64::from(i32::MAX));
        }
        *self.config.lock().unwrap() = config;
    }

    pub(crate) fn use_held(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        hand: Hand,
        stack: &ItemStack,
    ) -> bool {
        if !GRAPPLING_HOOK.matches(host, stack) {
            return false;
        }
        if research::can_use(host, player, &GRAPPLING_RESEARCH) {
            self.launch(host, player, hand, stack);
        }
        true
    }

    pub(crate) fn launch(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        hand: Hand,
        stack: &ItemStack,
    ) -> Option<()> {
        if self.shots.lock().unwrap().contains_key(&player.uuid()) {
            return None;
        }
        let mode = player.game_mode(host)?;
        if mode == GameMode::Spectator {
            return None;
        }
        let slot = if hand == Hand::Off {
            40
        } else {
            u32::try_from(player.selected_slot(host)?).ok()?
        };
        let slots = player.inventory_slots(host)?;
        if !slots.iter().any(|(index, current)| {
            *index == slot && current.same_item(stack) && current.same_components(stack)
        }) || slots
            .iter()
            .any(|(index, offhand)| *index == 40 && offhand.item().id(host) == "minecraft:bow")
        {
            return None;
        }
        let level = player.level(host)?;
        let time = level.game_time(host)?;
        let dimension = level.dimension().id(host)?;
        let owner = player.entity();
        let origin = owner.position(host)?;
        let eyes = owner.eye_position(host)?;
        let motion = owner.view_vector(host)?.map(|v| v * 2.0);
        let muzzle = std::array::from_fn(|i| eyes[i] + motion[i]);
        let invisibility = host.effect_key("minecraft:invisibility")?;
        let arrow_id = host.summon(&dimension, "minecraft:arrow", muzzle, tagged(ARROW_TAG))?;
        let arrow = host.entity(arrow_id)?;
        let Some(projectile) = host.projectile(arrow) else {
            arrow.discard(host);
            return None;
        };
        if !projectile.set_owner(host, Some(owner))
            || !projectile.set_pickup(host, ArrowPickup::Disallowed)
            || !arrow.set_velocity(host, motion)
        {
            arrow.discard(host);
            return None;
        }
        let mut bat_data = tagged(BAT_TAG);
        for name in ["NoAI", "Silent", "Invulnerable"] {
            bat_data.insert(name.into(), Tag::Byte(1));
        }
        bat_data.insert("CanPickUpLoot".into(), Tag::Byte(0));
        let Some(bat) = host
            .summon(&dimension, "minecraft:bat", origin, bat_data)
            .and_then(|id| host.entity(id))
        else {
            arrow.discard(host);
            return None;
        };
        let config = *self.config.lock().unwrap();
        let spent = config.consume
            && mode != GameMode::Creative
            && stack.item().id(host) == "minecraft:lead";
        let tether = Tether {
            arrow,
            bat,
            owner,
            spent,
            check_hit_at: None,
        };
        let hidden = ActiveEffect::new(invisibility, -1, 0, false, false, false);
        if !host
            .living(bat)
            .is_some_and(|living| living.add_effect(host, &hidden))
            || !bat.leash_to(host, arrow)
            || (spent && !item_clicks::consume_one(host, player, hand, stack))
        {
            remove_tether(host, tether);
            return None;
        }
        self.shots.lock().unwrap().insert(
            player.uuid(),
            Shot {
                level,
                until: time.saturating_add(config.lifespan),
                tether: Some(tether),
            },
        );
        Some(())
    }

    pub(crate) fn impact(&self, host: &dyn Host, event: &ProjectileImpactEvent<'_>) {
        if event.cancelled || event.kind != "minecraft:arrow" || event.entity.is_some() {
            return;
        }
        let mut shots = self.shots.lock().unwrap();
        let Some(shot) = shots.values_mut().find(|shot| {
            shot.level.dimension().id(host).as_deref() == Some(event.dimension)
                && shot.tether.is_some_and(|tether| {
                    tether.arrow.id() == event.projectile && tether.owner.id() == event.owner
                })
        }) else {
            return;
        };
        if let (Some(tether), Some(time)) = (&mut shot.tether, shot.level.game_time(host)) {
            // An earlier collision attempt may have been vetoed.
            tether.check_hit_at = Some(time.saturating_add(2));
        }
    }

    pub(crate) fn contact(&self, host: &mut dyn ServerHost, event: &ArrowContactPostEvent<'_>) {
        if !matches!(event.contact, "damage" | "hanging_break" | "portal") {
            return;
        }
        let active = self
            .shots
            .lock()
            .unwrap()
            .iter()
            .find_map(|(&uuid, &shot)| {
                let tether = shot.tether?;
                (tether.arrow.id() == event.projectile && tether.owner.id() == event.owner)
                    .then_some((uuid, shot, tether))
            });
        let Some((uuid, shot, tether)) = active else {
            return;
        };
        if shot.level.dimension().id(host).as_deref() != Some(event.dimension) {
            return;
        }
        let Some(player) = active_player(host, uuid, shot.level, tether.owner) else {
            self.finish(host, uuid, shot, None);
            return;
        };
        if let Some(time) = shot.level.game_time(host)
            && time < shot.until
        {
            self.anchor(host, player, shot, tether, time, event.at);
        }
    }

    fn anchor(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        shot: Shot,
        tether: Tether,
        time: i64,
        at: [f64; 3],
    ) {
        self.finish(host, player.uuid(), shot, Some(time.saturating_add(20)));
        if tether.spent
            && let Some(stack) = GRAPPLING_HOOK.stack(host, 1)
        {
            let _ = shot.level.spawn_item(host, at[0], at[1], at[2], stack, 16);
        }
        pull(host, shot.level, player, at);
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let Some(time) = level.game_time(host) else {
            return;
        };
        let shots: Vec<_> = self
            .shots
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, shot)| shot.level == level)
            .map(|(&uuid, &shot)| (uuid, shot))
            .collect();
        for (uuid, shot) in shots {
            if time >= shot.until {
                let cooldown = shot.tether.map(|_| time.saturating_add(20));
                self.finish(host, uuid, shot, cooldown);
                continue;
            }
            let Some(tether) = shot.tether else { continue };
            let Some(player) = active_player(host, uuid, level, tether.owner) else {
                self.finish(host, uuid, shot, None);
                continue;
            };
            if !host
                .projectile(tether.arrow)
                .is_some_and(|arrow| arrow.owner(host) == Some(tether.owner))
            {
                self.finish(host, uuid, shot, Some(shot.until.saturating_add(20)));
                continue;
            }
            if tether.check_hit_at.is_some_and(|hit| time >= hit)
                && tether
                    .arrow
                    .nbt(host)
                    .is_some_and(|nbt| nbt.get(&"inGround".into()) == Some(&Tag::Byte(1)))
                && let Some(at) = tether.arrow.position(host)
            {
                self.anchor(host, player, shot, tether, time, at);
            }
        }
    }

    fn finish(&self, host: &mut dyn ServerHost, uuid: u128, shot: Shot, cooldown: Option<i64>) {
        {
            let mut shots = self.shots.lock().unwrap();
            if let Some(until) = cooldown {
                shots.insert(
                    uuid,
                    Shot {
                        level: shot.level,
                        until,
                        tether: None,
                    },
                );
            } else {
                shots.remove(&uuid);
            }
        }
        if let Some(tether) = shot.tether {
            remove_tether(host, tether);
        }
    }

    pub(crate) fn abandon(&self, host: &mut dyn ServerHost, uuid: u128) {
        let Some(shot) = self.shots.lock().unwrap().get(&uuid).copied() else {
            return;
        };
        if shot.level.in_place(host) {
            self.finish(host, uuid, shot, None);
        }
    }

    pub(crate) fn unload(&self, host: &mut dyn ServerHost, dimension: &str) {
        let shots: Vec<_> = self
            .shots
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, shot)| shot.level.dimension().id(host).as_deref() == Some(dimension))
            .map(|(&uuid, &shot)| (uuid, shot))
            .collect();
        for (uuid, shot) in shots {
            self.finish(host, uuid, shot, None);
        }
    }

    pub(crate) fn clear(&self, host: &mut dyn ServerHost) {
        let shots = std::mem::take(&mut *self.shots.lock().unwrap());
        for shot in shots.values() {
            if let Some(tether) = shot.tether {
                remove_tether(host, tether);
            }
        }
    }
}

fn active_player(
    host: &dyn Host,
    uuid: u128,
    level: LevelRef,
    owner: EntityRef,
) -> Option<PlayerRef> {
    host.player(uuid).filter(|player| {
        player.level(host) == Some(level)
            && player.entity() == owner
            && player
                .game_mode(host)
                .is_some_and(|mode| mode != GameMode::Spectator)
    })
}

pub(crate) fn main_hand_has_hook(host: &dyn Host, uuid: u128) -> bool {
    let Some(player) = host.player(uuid) else {
        return false;
    };
    let Some(selected) = player.selected_slot(host) else {
        return false;
    };
    player.inventory_slots(host).is_some_and(|slots| {
        slots
            .iter()
            .any(|(slot, stack)| *slot as usize == selected && GRAPPLING_HOOK.matches(host, stack))
    })
}

pub(crate) fn expired_saved_entity(host: &dyn Host, event: &EntityJoinLevelEvent<'_>) -> bool {
    if !event.loaded_from_disk {
        return false;
    }
    let expected = match event.kind {
        "minecraft:arrow" => ARROW_TAG,
        "minecraft:bat" => BAT_TAG,
        _ => return false,
    };
    host.entity(event.entity)
        .and_then(|entity| entity.tags(host))
        .is_some_and(|tags| tags.iter().any(|tag| tag == expected))
}

fn tagged(tag: &str) -> Compound {
    let mut data = Compound::new();
    data.insert("Tags".into(), Tag::List(vec![Tag::String(tag.into())]));
    data
}

fn remove_tether(host: &mut dyn ServerHost, tether: Tether) {
    let _ = tether.bat.detach_leash(host, false);
    let _ = tether.bat.discard(host);
    let _ = tether.arrow.discard(host);
}

fn pull(host: &mut dyn ServerHost, level: LevelRef, player: PlayerRef, target: [f64; 3]) {
    let Some(origin) = player.entity().position(host) else {
        return;
    };
    let mut delta: [f64; 3] = std::array::from_fn(|i| target[i] - origin[i]);
    if squared_length(delta) >= 9.0 {
        let Some(dimension) = level.dimension().id(host) else {
            return;
        };
        let lift = [origin[0], origin[1] + 0.5, origin[2]];
        if player.teleport_checked(host, &dimension, lift, None) != Ok(PlayerTeleportStatus::Moved)
        {
            return;
        }
        delta = std::array::from_fn(|i| target[i] - lift[i]);
        delta = ballistic(delta);
    } else if delta[1] > 0.0 {
        delta = [0.0, 0.2, 0.0];
    }
    let _ = player.entity().set_velocity(host, delta);
}

fn squared_length(delta: [f64; 3]) -> f64 {
    delta
        .into_iter()
        .map(|component| component * component)
        .sum()
}

fn ballistic([x, y, z]: [f64; 3]) -> [f64; 3] {
    let t = squared_length([x, y, z]).sqrt();
    [
        (1.0 + 0.08 * t) * x / t,
        (1.0 + 0.04 * t) * y / t + 0.04 * t,
        (1.0 + 0.08 * t) * z / t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Kind;

    #[test]
    fn paper_chain_and_ballistic_pull_use_original_values() {
        let ids = |i: usize| CRAFTING[i].inputs.map(|kind| kind.map_or("", Kind::id));
        assert_eq!(
            ids(0),
            [
                "",
                "STEEL_INGOT",
                "",
                "STEEL_INGOT",
                "",
                "STEEL_INGOT",
                "",
                "",
                ""
            ]
        );
        assert_eq!(ids(1), ["", "", "HOOK", "", "CHAIN", "", "CHAIN", "", ""]);
        let launched = ballistic([6.0, 0.0, 8.0]);
        for (got, want) in launched.into_iter().zip([1.08, 0.4, 1.44]) {
            assert!((got - want).abs() < 1e-10);
        }
    }
}
