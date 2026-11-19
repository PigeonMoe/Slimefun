use mod_api::{Flow, GameMode, PlayerFallReactionEvent, PlayerFallReactionPostEvent, ServerHost};

const STOMP: u128 = 0x75a1_9d82_8a70_4c3e_bbe8_0432_16c5_b1cd;
const BLOCK_FACES: [[i32; 3]; 19] = [
    [0, 0, -1],
    [1, 0, 0],
    [0, 0, 1],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [1, 0, -1],
    [-1, 0, -1],
    [1, 0, 1],
    [-1, 0, 1],
    [-2, 0, -1],
    [-1, 0, -2],
    [1, 0, -2],
    [2, 0, -1],
    [2, 0, 1],
    [1, 0, 2],
    [-1, 0, 2],
    [-2, 0, 1],
    [0, 0, 0],
];

pub(crate) fn select(event: &mut PlayerFallReactionEvent<'_>) -> Flow {
    if !crate::armor_forge::stomp_unlocked(event.boots, event.persisted) {
        return Flow::Continue;
    }
    *event.effect = (
        STOMP,
        Some(("minecraft:entity.zombie.break_wooden_door".into(), 2.0)),
    );
    Flow::Cancel
}

pub(crate) fn apply(host: &mut dyn ServerHost, event: &PlayerFallReactionPostEvent) {
    if event.reaction != STOMP {
        return;
    }
    let player = event.player.entity();
    let Some(level) = player.level(host) else {
        return;
    };
    player.set_velocity(host, [0.0, 0.7, 0.0]);
    let [min, max] = event.bounds;
    let min = min.map(|value| value - 4.0);
    let max = max.map(|value| value + 4.0);
    let mut nearby = level.entities_in(host, min, max).unwrap_or_default();
    // Bukkit's entity query includes spectators, who can have a live collision flag.
    nearby.extend(
        level
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
    for target in nearby {
        if target == player || target.no_gravity(host) == Some(true) {
            continue;
        }
        let Some(living) = host.living(target) else {
            continue;
        };
        if living.health(host).is_none_or(|health| health <= 0.0) {
            continue;
        }
        let Some(position) = target.position(host) else {
            continue;
        };
        target.set_velocity(host, shockwave(event.origin, position));
        if target.allows_player_damage(host, event.player) == Some(true) {
            target.hurt(host, "minecraft:generic", event.damage / 2.0, None, None);
        }
    }
    let [x, y, z] = event.origin.map(|value| value.floor() as i32);
    for [dx, dy, dz] in BLOCK_FACES {
        let pos = [x + dx, y - 1 + dy, z + dz];
        if let Some(material) = level.block(host, pos)
            && let Some(state) = host.block_state_default(material)
        {
            level.block_break_effect(host, pos, state);
        }
    }
}

fn shockwave(origin: [f64; 3], target: [f64; 3]) -> [f64; 3] {
    let delta: [f64; 3] = std::array::from_fn(|axis| target[axis] - origin[axis]);
    let squared: f64 = delta.iter().map(|v| v * v).sum();
    if squared < 0.05 {
        [0.0, 1.0, 0.0]
    } else {
        let distance = squared.sqrt();
        delta.map(|v| v / distance * 1.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shockwave_is_absolute_and_uses_papers_close_range_cutoff() {
        assert_eq!(shockwave([0.0, 0.0, 0.0], [0.2, 0.0, 0.0]), [0.0, 1.0, 0.0]);
        assert_eq!(
            shockwave([0.0, 0.0, 0.0], [0.25, 0.0, 0.0]),
            [1.4, 0.0, 0.0]
        );
        assert_eq!(
            shockwave([7.0, 2.0, 1.0], [7.0, -1.0, 1.0]),
            [0.0, -1.4, 0.0]
        );
        let angled = shockwave([0.0, 0.0, 0.0], [0.0, 3.0, -4.0]);
        assert!((angled[1] - 0.84).abs() < 1.0e-12);
        assert!((angled[2] + 1.12).abs() < 1.0e-12);
    }
}
