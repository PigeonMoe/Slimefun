// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{GameMode, LevelRef, ParticleKey, ParticleOptions, ServerHost, SoundSource};

#[derive(Clone, Copy)]
pub(super) struct Blast {
    pub level: LevelRef,
    pub projectile: u32,
    pub owner: u32,
    pub victim: u32,
    pub origin: [f64; 3],
    pub damage: f32,
}

impl Blast {
    pub(super) fn apply(
        self,
        host: &mut dyn ServerHost,
        range: i32,
        particle: Option<ParticleKey>,
    ) {
        if let Some(particle) = particle {
            let _ = self.level.send_particles(
                host,
                &ParticleOptions::simple(particle),
                None,
                self.origin,
                [0.0; 3],
                0.0,
                1,
            );
        }
        let _ = self.level.play_sound(
            host,
            "minecraft:entity.generic.explode",
            self.origin,
            SoundSource::Players,
            1.0,
            1.0,
        );
        let Some(owner) = host.entity(self.owner) else {
            return;
        };
        let Some(player) = owner
            .uuid(host)
            .and_then(|uuid| host.player(uuid))
            .filter(|player| player.entity() == owner)
        else {
            return;
        };

        let radius = f64::from(range);
        let min = self.origin.map(|axis| axis - radius);
        let max = self.origin.map(|axis| axis + radius);
        let mut targets = self.level.entities_in(host, min, max).unwrap_or_default();
        // Paper's nearby-entity query also sees spectators.
        targets.extend(
            self.level
                .online_players(host)
                .unwrap_or_default()
                .into_iter()
                .filter(|player| player.game_mode(host) == Some(GameMode::Spectator))
                .map(|player| player.entity())
                .filter(|entity| {
                    entity.bounding_box(host).is_some_and(|[lo, hi]| {
                        (0..3).all(|axis| min[axis] < hi[axis] && max[axis] > lo[axis])
                    })
                }),
        );
        for target in targets {
            if target.id() == self.victim
                || target
                    .kind(host)
                    .is_some_and(|kind| kind.id(host) == "minecraft:armor_stand")
                || host
                    .living(target)
                    .and_then(|living| living.health(host))
                    .is_none_or(|health| health <= 0.0)
            {
                continue;
            }
            let Some(position) = target.position(host) else {
                continue;
            };
            if target.allows_player_damage(host, player) != Some(true) {
                continue;
            }
            let Some(velocity) = target.velocity(host) else {
                continue;
            };
            let (amount, push) = splash(self.origin, position, range, self.damage);
            // Paper asks protection with the arrow's shooter, then applies a generic hurt.
            let _ = target.set_velocity(
                host,
                std::array::from_fn(|axis| velocity[axis] + push[axis]),
            );
            let _ = target.hurt(host, "minecraft:generic", amount, None, None);
        }
    }
}

fn splash(origin: [f64; 3], target: [f64; 3], range: i32, damage: f32) -> (f32, [f64; 3]) {
    let [x, y, z] = std::array::from_fn(|axis| target[axis] - origin[axis]);
    let distance_squared = x * x + (y + 0.75) * (y + 0.75) + z * z;
    let denominator = f64::from(2_i32.wrapping_mul(range).wrapping_mul(range));
    let amount = f64::from(damage) * (1.0 - distance_squared / denominator);
    let scale = 2.0 / (x * x + 0.75 * 0.75 + z * z).sqrt();
    (amount as f32, [x * scale, 0.75 * scale, z * scale])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_uses_height_for_damage_and_fixed_lift_for_additive_knockback() {
        let origin = [0.0, 64.0, 0.0];
        let (level, push) = splash(origin, [1.0, 64.0, 0.0], 3, 9.0);
        assert_eq!(level, 8.21875);
        for (actual, expected) in push.into_iter().zip([1.6, 1.2, 0.0]) {
            assert!((actual - expected).abs() < 1.0e-12);
        }
        let (above, raised) = splash(origin, [1.0, 66.0, 0.0], 3, 9.0);
        assert_eq!(above, 4.71875);
        assert_eq!(raised, push);
        let (corner, _) = splash(origin, [3.0, 67.0, 3.0], 3, 9.0);
        assert_eq!(corner, -7.03125);
    }
}
