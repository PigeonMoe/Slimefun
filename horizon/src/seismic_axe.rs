// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeSet;

use mod_api::{EntityRef, Falling, Hand, ItemStack, LevelRef, PlayerRef, ServerHost, Tag, Text};

use crate::{
    elemental_staves::STAFF_ELEMENTAL,
    items::{HARDENED_METAL_INGOT, item},
    ore_tools::paper_tool_wear,
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(SEISMIC_AXE, "iron_axe", "Seismic Axe", "green", None, None);

pub(crate) const RESEARCH: Research = Research {
    id: "seismic_axe",
    name: "Seismic Axe",
    cost: 29,
};

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(HARDENED_METAL_INGOT),
        Some(HARDENED_METAL_INGOT),
        None,
        Some(HARDENED_METAL_INGOT),
        Some(STAFF_ELEMENTAL),
        None,
        None,
        Some(STAFF_ELEMENTAL),
        None,
    ],
    output: Stack {
        kind: SEISMIC_AXE,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn lore() -> Tag {
    Tag::List(
        [
            "",
            "&7&oA portable Earthquake...",
            "",
            "&eRight Click&7 to use",
        ]
        .into_iter()
        .map(crate::hologram_projector::colored_lore)
        .collect(),
    )
}

type Cell = [i32; 3];
type Point = [f64; 3];

const RANGE: f64 = 10.0;
const GRID: i64 = 1 << 24;

fn shift(cell: &mut Cell, axis: usize, step: [i32; 3]) {
    cell[axis] += step[axis];
}

// Bukkit's BlockIterator takes one step on the dominant axis, then any
// crossed secondary axes in crossing order. At a tie the third axis wins.
fn sight_cells(eye: Point, direction: Point) -> Vec<Cell> {
    let speed = direction.map(f64::abs);
    let mut main = 0;
    for axis in 1..3 {
        if speed[axis] > speed[main] {
            main = axis;
        }
    }
    if speed[main] == 0.0 || !eye.into_iter().chain(direction).all(f64::is_finite) {
        return Vec::new();
    }
    let side = [(main + 1) % 3, (main + 2) % 3];
    let sign = direction.map(|axis| if axis > 0.0 { 1 } else { -1 });
    let start = eye.map(|coordinate| coordinate.floor() as i32);
    let fraction = |axis: usize| {
        let from_low = eye[axis] - f64::from(start[axis]);
        if sign[axis] > 0 {
            from_low
        } else {
            1.0 - from_low
        }
    };
    let back = fraction(main) / speed[main];
    let side_step = side.map(|axis| (speed[axis] / speed[main] * GRID as f64).round() as i64);
    let mut error =
        side.map(|axis| ((fraction(axis) - speed[axis] * back) * GRID as f64).floor() as i64);
    // Seed one slice behind the eye and trim all cells before the eye's cell.
    let mut cell = start;
    shift(&mut cell, main, sign.map(|v| -v));
    for i in 0..2 {
        error[i] = error[i].max(1 - side_step[i]);
        if error[i] < 0 {
            shift(&mut cell, side[i], sign.map(|v| -v));
        } else {
            error[i] -= GRID;
        }
    }
    let length = (speed[main] * speed[main]
        + speed[side[0]] * speed[side[0]]
        + speed[side[1]] * speed[side[1]])
        .sqrt();
    let slices = (RANGE / (length / speed[main])).round() as usize;
    let mut cells = Vec::with_capacity(1 + slices * 3);
    for _ in 0..=slices {
        shift(&mut cell, main, sign);
        cells.push(cell);
        for i in 0..2 {
            error[i] += side_step[i];
        }
        let order = if side_step[0] * error[1] < side_step[1] * error[0] {
            [0, 1]
        } else {
            [1, 0]
        };
        for i in order {
            if error[i] > 0 {
                shift(&mut cell, side[i], sign);
                cells.push(cell);
                error[i] -= GRID;
            }
        }
    }
    let Some(first) = cells.iter().position(|&cell| cell == start) else {
        return Vec::new();
    };
    cells.drain(..first);
    cells
}

fn direction([yaw, pitch]: [f32; 2]) -> Point {
    let (sin_yaw, cos_yaw) = f64::from(yaw).to_radians().sin_cos();
    let (sin_pitch, cos_pitch) = f64::from(pitch).to_radians().sin_cos();
    [-cos_pitch * sin_yaw, -sin_pitch, cos_pitch * cos_yaw]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Material {
    Air,
    OtherAir,
    Solid,
}

fn line_of_sight(
    cells: impl IntoIterator<Item = Cell>,
    mut material: impl FnMut(Cell) -> Option<Material>,
) -> Vec<Cell> {
    let mut line = Vec::new();
    for pos in cells {
        let Some(block) = material(pos) else { break };
        line.push(pos);
        if block == Material::Solid {
            break;
        }
    }
    line
}

fn ground_at(pos: Cell, bottom: i32, mut material: impl FnMut(Cell) -> Option<Material>) -> Cell {
    // Paper descends only through ordinary Material.AIR and excludes minHeight.
    let [x, y, z] = pos;
    if material(pos) != Some(Material::Air) {
        return pos;
    }
    (bottom + 1..y)
        .rev()
        .map(|y| [x, y, z])
        .find(|&below| material(below) != Some(Material::Air))
        .unwrap_or(pos)
}

fn squared(vector: Point) -> f64 {
    vector
        .into_iter()
        .map(|component| component * component)
        .sum()
}

fn shove(source: Point, target: Point, ground: Cell) -> Option<Point> {
    let delta = std::array::from_fn(|axis| target[axis] - source[axis]);
    let distance = squared(delta);
    let ground_distance = squared(std::array::from_fn(|axis| {
        target[axis] - f64::from(ground[axis])
    }));
    if distance <= f64::from(0.2_f32 * 0.2_f32) || ground_distance >= 1.5 * 1.5 {
        return None;
    }
    let length = distance.sqrt();
    let strength = f64::from(1.2_f32);
    Some([
        delta[0] / length * strength,
        f64::from(0.9_f32),
        delta[2] / length * strength,
    ])
}

fn chunk([x, _, z]: Cell) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

fn hit_targets(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    source: Point,
    ground: Cell,
    targets: &[EntityRef],
    pushed: &mut BTreeSet<u32>,
) {
    let selected_chunk = chunk(ground);
    for &target in targets {
        let (Some(position), Some(cell)) = (target.position(host), target.block_position(host))
        else {
            continue;
        };
        if chunk(cell) != selected_chunk {
            continue;
        }
        if let Some(motion) = shove(source, position, ground)
            && pushed.insert(target.id())
            && target.allows_player_damage(host, player) == Some(true)
        {
            target.set_velocity(host, motion);
            // The Paper synthetic attack attributes the permission check to the
            // player; its subsequent LivingEntity.damage(double) is sourceless.
            target.hurt(host, "minecraft:generic", 6.0, None, None);
        }
    }
}

fn shockwave(host: &mut dyn ServerHost, player: PlayerRef, level: LevelRef) -> Option<()> {
    let entity = player.entity();
    let source = entity.position(host)?;
    let view = direction(entity.rotation(host)?);
    let (bottom, height) = level.height(host)?;
    let dimension = level.dimension().id(host)?;
    let air = host.block_key("minecraft:air")?;
    let alternate_air = [
        host.block_key("minecraft:cave_air")?,
        host.block_key("minecraft:void_air")?,
    ];
    let material_at = |host: &dyn mod_api::Host, cell: Cell| {
        if cell[1] < bottom || cell[1] >= bottom + height {
            return Some(Material::OtherAir);
        }
        let block = level.block(host, cell)?;
        Some(if block == air {
            Material::Air
        } else if alternate_air.contains(&block) {
            Material::OtherAir
        } else {
            Material::Solid
        })
    };
    let line = line_of_sight(sight_cells(entity.eye_position(host)?, view), |cell| {
        material_at(host, cell)
    });
    let targets = level
        .all_entities(host)
        .unwrap_or_default()
        .into_iter()
        .filter(|&target| {
            target != entity
                && host.living(target).is_some()
                && target
                    .kind(host)
                    .is_some_and(|kind| kind.id(host) != "minecraft:armor_stand")
        })
        .collect::<Vec<_>>();
    let mut pushed = BTreeSet::new();
    for (index, cell) in line.into_iter().enumerate().skip(2) {
        let ground = ground_at(cell, bottom, |pos| material_at(host, pos));
        if let (Some(state), Some(material)) =
            (level.block_state(host, ground), level.block(host, ground))
        {
            if let Some(appearance) = host.block_state_default(material) {
                level.block_break_effect(host, ground, appearance);
            }
            let [x, y, z] = ground;
            if matches!(
                material_at(host, [x, y + 1, z]),
                Some(Material::Air | Material::OtherAir)
            ) {
                host.spawn_falling_block(
                    &dimension,
                    state,
                    [f64::from(x) + 0.5, f64::from(y + 1), f64::from(z) + 0.5],
                    [0.0, 0.4 + index as f64 * 0.01, 0.0],
                    Falling::ghost(),
                );
            }
        }
        hit_targets(host, player, source, ground, &targets, &mut pushed);
    }
    Some(())
}

/// True lets the ordinary right-click phases continue.
pub(crate) fn activate(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    stack: &ItemStack,
    rolls: &Rolls,
) -> bool {
    if !research::can_use(host, player, &RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock Seismic Axe with /sf research seismic_axe."),
        );
        return false;
    }
    let Some(level) = player.level(host) else {
        return true;
    };
    shockwave(host, player, level);
    let mut actual = stack.clone();
    for _ in 0..4 {
        let Some(next) = paper_tool_wear(host, player, level, hand, &actual, rolls) else {
            break;
        };
        actual = next;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traverses_the_paper_block_iterator_cells_and_includes_the_first_obstacle() {
        // Captured from Paper 26.2 build 132's org.bukkit.util.BlockIterator.
        assert_eq!(
            sight_cells([0.0, 64.0, 0.0], [1.0, 1.0, 1.0]),
            [
                [0, 64, 0],
                [1, 64, 0],
                [1, 64, 1],
                [1, 65, 1],
                [2, 65, 1],
                [2, 65, 2],
                [2, 66, 2],
                [3, 66, 2],
                [3, 66, 3],
                [3, 67, 3],
                [4, 67, 3],
                [4, 67, 4],
                [4, 68, 4],
                [5, 68, 4],
                [5, 68, 5],
                [5, 69, 5],
                [6, 69, 5],
                [6, 69, 6],
                [6, 70, 6],
            ]
        );
        assert_eq!(
            sight_cells([-2.75, 0.5, 5.125], [-1.0, 0.35, 0.5]),
            [
                [-3, 0, 5],
                [-4, 0, 5],
                [-5, 0, 5],
                [-5, 1, 5],
                [-5, 1, 6],
                [-6, 1, 6],
                [-7, 1, 6],
                [-7, 1, 7],
                [-8, 1, 7],
                [-8, 2, 7],
                [-9, 2, 7],
                [-9, 2, 8],
                [-10, 2, 8],
                [-10, 3, 8],
                [-11, 3, 8],
                [-11, 3, 9],
                [-12, 3, 9],
            ]
        );
        let east = sight_cells([0.5, 64.62, 0.5], [1.0, 0.0, 0.0]);
        let line = line_of_sight(east, |[x, _, _]| {
            Some(match x {
                1 | 2 => Material::OtherAir,
                3 => Material::Solid,
                _ => Material::Air,
            })
        });
        assert_eq!(line, [[0, 64, 0], [1, 64, 0], [2, 64, 0], [3, 64, 0]]);
    }

    #[test]
    fn grounds_only_through_ordinary_air_and_uses_paper_distance_limits() {
        assert_eq!(
            ground_at([4, 8, 2], -64, |[_, y, _]| Some(match y {
                3 => Material::OtherAir,
                2 => Material::Solid,
                _ => Material::Air,
            })),
            [4, 3, 2]
        );
        assert_eq!(
            ground_at([4, -62, 2], -64, |[_, y, _]| Some(if y == -64 {
                Material::Solid
            } else {
                Material::Air
            })),
            [4, -62, 2]
        );
        assert_eq!(
            shove([3.0, 9.0, -1.0], [4.0, 9.0, -1.0], [4, 8, -1]),
            Some([f64::from(1.2_f32), f64::from(0.9_f32), 0.0,])
        );
        assert_eq!(shove([0.0, 0.0, 0.0], [0.15, 0.0, 0.0], [0, 0, 0]), None);
        assert_eq!(shove([0.0, 0.0, 0.0], [1.5, 0.0, 0.0], [0, 0, 0]), None);
    }
}
