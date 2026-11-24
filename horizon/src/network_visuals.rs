// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::OnceLock};

use mod_api::{
    Compound, EntityRef, LevelRef, ParticleKey, ParticleOptions, Registrar, RemovalReason,
    ServerHost, Tag, Text,
};

use super::Position;
use crate::geo_deposits::{Mining, Resource};

const HOLOGRAM: &str = "slimefun.network_status";
static DUST: OnceLock<ParticleKey> = OnceLock::new();

pub(super) fn register(registrar: &mut dyn Registrar) {
    let _ = DUST.set(registrar.particle_key("minecraft:dust"));
}

#[derive(Clone, Copy)]
pub(super) enum Status {
    CargoOnline,
    CargoEmpty,
    EnergyEmpty,
    CargoConflict,
    EnergyConflict,
    Energy { supply: u64, demand: u64 },
    ReactorCooling(u32),
    GeoIdle,
    GeoNeedsScan,
    GeoMining(Resource),
    GeoFinished,
}

impl Status {
    fn is_geo_miner(self) -> bool {
        matches!(
            self,
            Self::GeoIdle | Self::GeoNeedsScan | Self::GeoMining(_) | Self::GeoFinished
        )
    }

    pub(super) fn geo_miner(
        operation: Option<Resource>,
        reserve: impl FnOnce() -> Option<Mining>,
    ) -> Self {
        if let Some(resource) = operation {
            return Self::GeoMining(resource);
        }
        match reserve() {
            Some(Mining::Unscanned) => Self::GeoNeedsScan,
            Some(Mining::Exhausted) => Self::GeoFinished,
            Some(Mining::Available(_)) | None => Self::GeoIdle,
        }
    }

    fn label(self) -> (String, Text) {
        match self {
            Self::CargoOnline => (
                "Status: ONLINE".into(),
                color("Status: ", "gray", false).append(color("ONLINE", "green", true)),
            ),
            Self::CargoEmpty => single_label("No Cargo Nodes found", "red"),
            Self::EnergyEmpty => single_label("No Energy Network found", "dark_red"),
            Self::CargoConflict => single_label("Multiple Cargo Regulators connected", "dark_red"),
            Self::EnergyConflict => {
                single_label("Multiple Energy Regulators connected", "dark_red")
            }
            Self::GeoIdle => single_label("Idling...", "gray"),
            Self::GeoNeedsScan => single_label("GEO-Scan required!", "dark_red"),
            Self::GeoMining(resource) => (
                format!("Mining: {}", resource.name()),
                color("Mining: ", "gray", false).append(color(resource.name(), "white", false)),
            ),
            Self::GeoFinished => single_label("Finished", "gray"),
            Self::ReactorCooling(percent) => {
                let percent = if percent == 100 {
                    "100%".into()
                } else {
                    format!("{percent}.0%")
                };
                (
                    format!("❄ {percent}"),
                    color("❄ ", "aqua", false).append(color(percent, "gray", false)),
                )
            }
            Self::Energy { supply, demand } => {
                let (sign, prefix, number) = if supply < demand {
                    ("- ", "dark_red", "red")
                } else {
                    ("+ ", "dark_green", "green")
                };
                let value = compact(supply.abs_diff(demand));
                (
                    format!("{sign}{value} J ⚡"),
                    Text::literal("")
                        .append(color(sign, prefix, true))
                        .append(color(value, number, false))
                        .append(color(" J ", "gray", false))
                        .append(color("⚡", "yellow", false)),
                )
            }
        }
    }
}

fn single_label(label: &str, name: &str) -> (String, Text) {
    (label.into(), color(label, name, false))
}

fn color(value: impl Into<String>, name: &str, bold: bool) -> Text {
    let mut text = Text::literal(value);
    text.style.color = Some(name.into());
    text.style.bold = Some(bold);
    text
}

pub(crate) fn compact(value: u64) -> String {
    for (divisor, suffix) in [(1_000_000_000, 'B'), (1_000_000, 'M'), (1_000, 'K')] {
        if value >= divisor {
            let decimal = format!("{:.2}", value as f64 / divisor as f64);
            return format!(
                "{}{suffix}",
                decimal.trim_end_matches('0').trim_end_matches('.')
            );
        }
    }
    value.to_string()
}

pub(super) fn toggled(host: &mut dyn ServerHost, player: mod_api::PlayerRef, disabled: bool) {
    let (symbol, shade) = if disabled {
        ("✘", "dark_red")
    } else {
        ("✔", "dark_green")
    };
    let message = color("Cargo Net Visualizer: ", "red", false).append(color(symbol, shade, false));
    let _ = player.tell(host, message);
}

pub(super) fn cargo_particles(
    host: &mut dyn ServerHost,
    level: LevelRef,
    nodes: impl Iterator<Item = Position>,
) {
    let Some(&key) = DUST.get() else {
        return;
    };
    let dust = ParticleOptions::dust(key, [0.0, 0.0, 1.0], 3.0);
    for pos in nodes {
        level.send_particles(
            host,
            &dust,
            None,
            pos.map(|n| f64::from(n) + 0.5),
            [0.0; 3],
            1.0,
            1,
        );
    }
}

pub(super) fn reconcile(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dimension: &str,
    labels: BTreeMap<Position, Option<Status>>,
) {
    reconcile_at(host, level, dimension, labels, None);
}

pub(super) fn remove_status(host: &mut dyn ServerHost, level: LevelRef, pos: Position) {
    if let Some(dimension) = level.dimension().id(host) {
        reconcile_at(host, level, &dimension, BTreeMap::new(), Some(pos));
    }
}

pub(super) fn geo_miner_placement(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    placed: bool,
) {
    if let Some(dimension) = level.dimension().id(host) {
        let label = placed.then_some((pos, Some(Status::GeoIdle)));
        reconcile_at(
            host,
            level,
            &dimension,
            label.into_iter().collect(),
            Some(pos),
        );
    }
}

fn reconcile_at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dimension: &str,
    mut labels: BTreeMap<Position, Option<Status>>,
    only: Option<Position>,
) {
    let Some(entities) = level.all_entities(host) else {
        return;
    };
    let recovering_paper = only.is_none()
        && labels
            .values()
            .flatten()
            .any(|status| status.is_geo_miner());
    let mut native = Vec::new();
    let mut paper_geo = BTreeMap::new();
    for entity in entities {
        if !entity
            .kind(host)
            .is_some_and(|kind| kind.id(host) == "minecraft:armor_stand")
        {
            continue;
        }
        let Some(tags) = entity.tags(host) else {
            continue;
        };
        let owned = tags.iter().any(|tag| tag == HOLOGRAM);
        if !owned && (!recovering_paper || !tags.is_empty()) {
            continue;
        }
        let Some(pos) = entity
            .block_position(host)
            .filter(|&pos| only.is_none_or(|target| pos == target) && level.is_loaded(host, pos))
        else {
            continue;
        };
        if owned {
            native.push((pos, entity));
        } else if labels
            .get(&pos)
            .copied()
            .flatten()
            .is_some_and(Status::is_geo_miner)
            && paper_geo_stand(host, entity, pos)
        {
            paper_geo
                .entry(pos)
                .and_modify(|found| *found = None)
                .or_insert(Some(entity));
        }
    }
    // Prefer the original entity even when a Horizon label was already spawned
    // before the Paper entity loaded. Only one verified candidate can be adopted.
    for (&pos, &candidate) in &paper_geo {
        if let Some(entity) = candidate
            && entity.add_tag(host, HOLOGRAM)
            && let Some(Some(status)) = labels.remove(&pos)
            && !matches!(status, Status::GeoIdle)
        {
            set_label(host, entity, status);
        }
    }
    for (pos, entity) in native {
        match labels.remove(&pos) {
            // Paper leaves the previous label when an unstarted recipe could
            // not fit. Spawn the idle label only if the marker is absent.
            Some(Some(Status::GeoIdle)) if only.is_none() => {}
            Some(Some(status)) => set_label(host, entity, status),
            Some(None) => {}
            None => {
                entity.remove(host, RemovalReason::Discarded);
            }
        }
    }
    // Preserve all ambiguous Paper markers and avoid creating another at that anchor.
    for pos in paper_geo.into_keys() {
        labels.remove(&pos);
    }
    for (pos, status) in labels
        .into_iter()
        .filter_map(|(pos, status)| Some((pos, status?)))
    {
        let mut center = pos.map(|n| f64::from(n) + 0.5);
        center[1] += 0.25;
        let mut properties =
            Compound::from([("Tags".into(), Tag::List(vec![Tag::String(HOLOGRAM.into())]))]);
        for flag in [
            "Invisible",
            "Invulnerable",
            "Silent",
            "Marker",
            "NoAI",
            "NoGravity",
        ] {
            properties.insert(flag.into(), Tag::Byte(1));
        }
        let Some(entity) = host
            .summon(dimension, "minecraft:armor_stand", center, properties)
            .and_then(|id| host.entity(id))
        else {
            continue;
        };
        set_label(host, entity, status);
    }
}

fn paper_geo_stand(host: &mut dyn ServerHost, entity: EntityRef, pos: Position) -> bool {
    let [x, y, z] = pos.map(f64::from);
    entity.position(host) == Some([x + 0.5, y + 0.75, z + 0.5])
        && entity.vehicle(host).is_none()
        && entity.is_vehicle(host) == Some(false)
        && entity.name(host).is_some_and(|name| paper_geo_name(&name))
        && entity
            .nbt(host)
            .is_some_and(|data| paper_geo_marker(&data, pos))
}

fn paper_geo_name(name: &str) -> bool {
    matches!(name, "Idling..." | "GEO-Scan required!" | "Finished")
        || name.strip_prefix("Mining: ").is_some_and(|name| {
            crate::geo_deposits::MINER_RESOURCES
                .into_iter()
                .any(|resource| name == resource.name())
        })
}

pub(crate) fn paper_hologram_id(values: &Compound, [x, y, z]: Position) -> bool {
    // Dough packs the signed X/Z in 26 bits each and the signed Y in 12 bits.
    const XZ: i32 = 1 << 25;
    if !(-XZ..XZ).contains(&x) || !(-XZ..XZ).contains(&z) || !(-2048..2048).contains(&y) {
        return false;
    }
    let packed = ((i64::from(x) & 0x3ff_ffff) << 38)
        | ((i64::from(z) & 0x3ff_ffff) << 12)
        | (i64::from(y) & 0xfff);
    values.get(&"slimefun:hologram_id".into()) == Some(&Tag::Long(packed))
}

fn paper_geo_marker(data: &Compound, pos: Position) -> bool {
    let Some(Tag::Compound(values)) = data.get(&"BukkitValues".into()) else {
        return false;
    };
    values.len() == 1
        && paper_hologram_id(values, pos)
        && [
            "Invisible",
            "Invulnerable",
            "Silent",
            "Marker",
            "NoGravity",
            "CustomNameVisible",
        ]
        .iter()
        .all(|&key| data.get(&key.into()) == Some(&Tag::Byte(1)))
        && ["equipment", "ArmorItems", "HandItems", "Passengers"]
            .iter()
            .all(|&key| !data.contains_key(&key.into()))
}

fn set_label(host: &mut dyn ServerHost, entity: mod_api::EntityRef, status: Status) {
    let (plain, label) = status.label();
    if entity.name(host).as_deref() != Some(&plain) {
        let _ = entity.set_custom_name(host, Some(label));
    }
    if entity.custom_name_visible(host) != Some(true) {
        entity.set_custom_name_visible(host, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_geo_marker_requires_the_paper_entity_pdc_and_intact_shape() {
        // These values came from the plugin JAR's shaded Dough BlockPosition.
        let stand = |packed| {
            let mut data = Compound::from([(
                "BukkitValues".into(),
                Tag::compound([("slimefun:hologram_id", Tag::Long(packed))]),
            )]);
            for key in [
                "Invisible",
                "Invulnerable",
                "Silent",
                "Marker",
                "NoGravity",
                "CustomNameVisible",
            ] {
                data.insert(key.into(), Tag::Byte(1));
            }
            data
        };
        let pos = [760, 110, 150];
        let mut valid = stand(208_907_209_891_950);
        assert!(paper_geo_marker(&valid, pos));
        assert!(!paper_geo_marker(&valid, [776, 110, 150]));
        assert!(!paper_geo_marker(&valid, [760, 110 + 4096, 150]));
        assert!(paper_geo_marker(&stand(-4_672_924_287_040), [-17, -64, 31]));
        for protected in ["equipment", "ArmorItems", "HandItems", "Passengers"] {
            let mut modified = valid.clone();
            modified.insert(protected.into(), Tag::List(vec![]));
            assert!(!paper_geo_marker(&modified, pos));
        }
        let Some(Tag::Compound(values)) = valid.get_mut(&"BukkitValues".into()) else {
            panic!("the fixture stores the entity PDC")
        };
        values.insert("other:entity-owner".into(), Tag::Long(23));
        assert!(!paper_geo_marker(&valid, pos));
        let mut wrong_domain = stand(208_907_209_891_950);
        let item_pdc = wrong_domain.remove(&"BukkitValues".into()).unwrap();
        wrong_domain.insert("PublicBukkitValues".into(), item_pdc);
        assert!(!paper_geo_marker(&wrong_domain, pos));
        assert!(paper_geo_name("Mining: Nether Ice"));
        assert!(!paper_geo_name("Mining: Oil"));
        assert!(!paper_geo_name("Status: ONLINE"));
    }

    #[test]
    fn geo_label_comes_from_the_committed_operation_before_the_chunk_reserve() {
        for (reserve, expected, shade) in [
            (Mining::Unscanned, "GEO-Scan required!", "dark_red"),
            (Mining::Available(Resource::Uranium), "Idling...", "gray"),
            (Mining::Exhausted, "Finished", "gray"),
        ] {
            let (plain, label) = Status::geo_miner(None, || Some(reserve)).label();
            assert_eq!(plain, expected);
            assert_eq!(label.style.color.as_deref(), Some(shade));
        }
        let (plain, label) = Status::geo_miner(Some(Resource::Uranium), || {
            panic!("an active operation precedes the chunk reserve, including its last item")
        })
        .label();
        assert_eq!(plain, "Mining: Small Chunks of Uranium");
        assert_eq!(label.style.color.as_deref(), Some("gray"));
        assert_eq!(label.children[0].style.color.as_deref(), Some("white"));
    }

    #[test]
    fn regulator_labels_match_paper_and_keep_coloured_components() {
        assert_eq!(Status::CargoOnline.label().0, "Status: ONLINE");
        let (plain, loss) = Status::Energy {
            supply: 30,
            demand: 1270,
        }
        .label();
        assert_eq!(plain, "- 1.24K J ⚡");
        assert_eq!(loss.children[0].style.color.as_deref(), Some("dark_red"));
        assert_eq!(loss.children[0].style.bold, Some(true));
        assert_eq!(
            Status::Energy {
                supply: 65,
                demand: 65
            }
            .label()
            .0,
            "+ 0 J ⚡"
        );
    }
}
