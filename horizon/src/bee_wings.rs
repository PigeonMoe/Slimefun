// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::HashMap, sync::Mutex};

use mod_api::{ActiveEffect, Heightmap, LevelRef, LivingRef, PlayerRef, ServerHost, Text};

use crate::{armor_forge::BEE_ARMOR, items, research};

#[derive(Default)]
pub(crate) struct BeeWings(Mutex<HashMap<u128, Glide>>);

struct Glide {
    first_check: i64,
    last_y: f64,
}

impl BeeWings {
    pub(crate) fn start(&self, host: &dyn ServerHost, uuid: u128) {
        let Some(player) = host.player(uuid) else {
            return;
        };
        let wearing_wings = player.inventory_slots(host).is_some_and(|slots| {
            slots.iter().any(|(slot, stack)| {
                *slot == 38 && items::item_id(stack).as_deref() == Some("BEE_WINGS")
            })
        });
        if !wearing_wings || !research::can_use(host, player, &BEE_ARMOR) {
            return;
        }
        if let Some((level, [_, y, _])) = player.level(host).zip(player.entity().position(host))
            && let Some(tick) = level.game_time(host)
        {
            self.0.lock().unwrap().insert(
                uuid,
                Glide {
                    first_check: tick + 3,
                    last_y: y,
                },
            );
        }
    }

    pub(crate) fn stop(&self, uuid: u128) {
        self.0.lock().unwrap().remove(&uuid);
    }

    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let mut flying = self.0.lock().unwrap();
        if flying.is_empty() {
            return;
        }
        let Some(now) = level.game_time(host) else {
            return;
        };
        let Some(effect) = host.effect_key("minecraft:slow_falling") else {
            return;
        };
        flying.retain(|uuid, glide| {
            let Some(player) = host.player(*uuid) else {
                return false;
            };
            if player.level(host) != Some(level) || now < glide.first_check {
                return true;
            }
            let living = LivingRef::new(player.entity());
            if living.is_gliding(host) != Some(true)
                || living.health(host).is_none_or(|health| health <= 0.0)
                || living.potion_affected(host, effect)
            {
                return false;
            }
            let Some([_, y, _]) = player.entity().position(host) else {
                return false;
            };
            if y < glide.last_y {
                let Some(distance) = surface_distance(host, player) else {
                    return true;
                };
                if in_range(distance, || solid_below(host, level, player)) {
                    let mut message = Text::literal(
                        "Your Bee Wings will help you to get back to the ground safe and slow",
                    );
                    message.style.color = Some("yellow".into());
                    let _ = player.tell(host, message);
                    let _ = player.entity().reset_fall_distance(host);
                    let active = ActiveEffect::new(effect, 60, 0, false, true, true);
                    let _ = living.add_effect(host, &active);
                } else if distance < 0 {
                    // Paper keeps the previous Y under an overhang when no floor is in range.
                    return true;
                }
            }
            glide.last_y = y;
            true
        });
    }
}

fn surface_distance(host: &dyn ServerHost, player: PlayerRef) -> Option<i32> {
    let [x, y, z] = player.entity().block_position(host)?;
    let heights = host
        .surface_heights(host.dimension(), &[(x, z)], Heightmap::WorldSurface)
        .ok()?;
    // Horizon exposes the first air height. Paper's getHighestBlockYAt returns the block below.
    let surface = heights.into_iter().next().flatten()? - 1;
    Some(y - surface)
}

fn solid_below(host: &dyn ServerHost, level: LevelRef, player: PlayerRef) -> bool {
    let Some([x, y, z]) = player.entity().block_position(host) else {
        return false;
    };
    (1..=6).any(|depth| {
        level
            .block_state(host, [x, y - depth, z])
            .is_some_and(|state| {
                host.block_in_tag(state.block(), "minecraft:blocks_motion") == Some(true)
            })
    })
}

fn in_range(distance: i32, solid_below: impl FnOnce() -> bool) -> bool {
    if distance < 0 {
        solid_below()
    } else {
        distance <= 4
    }
}

#[cfg(test)]
mod tests {
    use super::in_range;

    #[test]
    fn checks_the_world_surface_or_the_floor_below_an_overhang() {
        assert!(in_range(4, || panic!("surface needs no scan")));
        assert!(!in_range(5, || panic!("surface needs no scan")));
        assert!(in_range(-1, || true));
        assert!(!in_range(-1, || false));
    }
}
