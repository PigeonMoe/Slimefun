// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Host, HostTransaction,
    LevelRef, ServerHost, Tag,
};

use crate::{
    heated_pressure::{BUCKET_OF_OIL, NETHER_ICE, SMALL_URANIUM},
    items::{Kind, SALT},
    random::Rolls,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resource {
    Oil,
    NetherIce,
    Uranium,
    Salt,
}

// Paper sorts scanner results by the localized resource names.
pub(crate) const SCANNER_RESOURCES: [Resource; 4] = [
    Resource::NetherIce,
    Resource::Oil,
    Resource::Salt,
    Resource::Uranium,
];

// Captured from Paper 26.2 with this branch's Dough KeyMap and default registrations.
pub(crate) const MINER_RESOURCES: [Resource; 3] =
    [Resource::Uranium, Resource::NetherIce, Resource::Salt];

impl Resource {
    pub fn paper_key(self) -> &'static str {
        match self {
            Self::Oil => "slimefun-oil",
            Self::NetherIce => "slimefun-nether_ice",
            Self::Uranium => "slimefun-uranium",
            Self::Salt => "slimefun-salt",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Oil => "Oil",
            Self::NetherIce => "Nether Ice",
            Self::Uranium => "Small Chunks of Uranium",
            Self::Salt => "Salt",
        }
    }

    pub fn item(self) -> Kind {
        match self {
            Self::Oil => BUCKET_OF_OIL,
            Self::NetherIce => NETHER_ICE,
            Self::Uranium => SMALL_URANIUM,
            Self::Salt => SALT,
        }
    }

    fn deposit(self) -> &'static AttachmentType<Reserve> {
        match self {
            Self::Oil => &OIL_DEPOSIT,
            Self::NetherIce => &NETHER_ICE_DEPOSIT,
            Self::Uranium => &URANIUM_DEPOSIT,
            Self::Salt => &SALT_DEPOSIT,
        }
    }

    fn base_supply(self, dimension: &str, biome: &str) -> u32 {
        match (self, dimension) {
            (Self::Oil, "minecraft:overworld") => oil_supply(biome),
            (Self::NetherIce, "minecraft:the_nether") => match biome {
                "minecraft:crimson_forest" | "minecraft:warped_forest" => 48,
                "minecraft:basalt_deltas" => 64,
                _ => 32,
            },
            (Self::Uranium, "minecraft:overworld") => match biome {
                "minecraft:beach" | "minecraft:desert" | "minecraft:stony_shore" => 5,
                "minecraft:jagged_peaks"
                | "minecraft:stony_peaks"
                | "minecraft:windswept_hills"
                | "minecraft:windswept_gravelly_hills" => 8,
                "minecraft:badlands"
                | "minecraft:eroded_badlands"
                | "minecraft:wooded_badlands"
                | "minecraft:dripstone_caves" => 12,
                "minecraft:basalt_deltas" => 16,
                _ => 4,
            },
            (Self::Salt, "minecraft:overworld" | "minecraft:the_nether") => match biome {
                "minecraft:swamp" => 20,
                "minecraft:beach"
                | "minecraft:windswept_gravelly_hills"
                | "minecraft:stony_shore"
                | "minecraft:stony_peaks"
                | "minecraft:dripstone_caves" => 40,
                "minecraft:ocean"
                | "minecraft:cold_ocean"
                | "minecraft:warm_ocean"
                | "minecraft:lukewarm_ocean"
                | "minecraft:deep_ocean"
                | "minecraft:deep_cold_ocean"
                | "minecraft:deep_lukewarm_ocean" => 60,
                _ if dimension == "minecraft:the_nether" => 8,
                _ => 6,
            },
            _ => 0,
        }
    }

    fn generate(
        self,
        dimension: &str,
        biome: &str,
        roll: impl FnOnce(u32) -> Option<u32>,
    ) -> Option<u32> {
        let base = self.base_supply(dimension, biome);
        if base == 0 {
            return Some(0);
        }
        let bound = match self {
            Self::Oil => 8,
            Self::NetherIce => 6,
            Self::Uranium => 2,
            Self::Salt => 18,
        };
        Some(base + roll(bound).filter(|&deviation| deviation < bound)?)
    }
}

const PAPER_IMPORT: &str = "paper_import";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Reserve {
    remaining: u32,
    paper_import: Option<u32>,
}

impl Reserve {
    fn encode(self, resource: Resource) -> Result<Compound, AttachmentError> {
        let number = |n| {
            i32::try_from(n)
                .map(Tag::Int)
                .map_err(|_| AttachmentError::Decode)
        };
        let mut saved = Compound::from([(resource.paper_key().into(), number(self.remaining)?)]);
        if let Some(imported) = self.paper_import {
            saved.insert(PAPER_IMPORT.into(), number(imported)?);
        }
        Ok(saved)
    }

    fn decode(saved: &Compound, resource: Resource) -> Result<Self, AttachmentError> {
        let number = |key: &str| match saved.get(&key.into()) {
            Some(Tag::Int(value)) => u32::try_from(*value).map_err(|_| AttachmentError::Decode),
            _ => Err(AttachmentError::Decode),
        };
        let remaining = number(resource.paper_key())?;
        let paper_import = saved
            .contains_key(&PAPER_IMPORT.into())
            .then(|| number(PAPER_IMPORT))
            .transpose()?;
        if paper_import.is_some_and(|original| remaining > original) {
            return Err(AttachmentError::Decode);
        }
        Ok(Self {
            remaining,
            paper_import,
        })
    }
}

macro_rules! deposit {
    ($attachment:ident, $name:literal, $resource:ident) => {
        static $attachment: AttachmentType<Reserve> = AttachmentType {
            namespace: "slimefun",
            name: $name,
            default: || Reserve {
                remaining: 0,
                paper_import: None,
            },
            codec: Some(AttachmentCodec {
                encode: |reserve| reserve.encode(Resource::$resource),
                decode: |saved| Reserve::decode(saved, Resource::$resource),
            }),
            copy_on_death: false,
            sync: false,
        };
    };
}

// Preserve the original Horizon oil attachment and encoded fields for existing worlds.
deposit!(OIL_DEPOSIT, "geo_oil", Oil);
deposit!(NETHER_ICE_DEPOSIT, "geo_nether_ice", NetherIce);
deposit!(URANIUM_DEPOSIT, "geo_uranium", Uranium);
deposit!(SALT_DEPOSIT, "geo_salt", Salt);

pub(crate) fn chunk([x, _, z]: [i32; 3]) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

fn holder(host: &dyn Host, level: LevelRef, chunk: [i32; 2]) -> Result<Holder, &'static str> {
    if !level.in_place(host) || host.chunk(level, chunk).is_none() {
        return Err("The GEO chunk must be loaded in the active dimension.");
    }
    Ok(Holder::Chunk(chunk))
}

fn read(
    host: &mut dyn ServerHost,
    holder: Holder,
    resource: Resource,
) -> Result<Option<Reserve>, &'static str> {
    resource
        .deposit()
        .get_existing(host, holder)
        .map_err(|_| "The GEO reserve could not be read.")
}

pub(crate) fn supply(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    resource: Resource,
) -> Result<Option<u32>, &'static str> {
    let holder = holder(host, level, chunk(pos))?;
    read(host, holder, resource).map(|reserve| reserve.map(|reserve| reserve.remaining))
}

/// Call within the same transaction that records the pending machine output.
pub(crate) fn reserve_one(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    resource: Resource,
) -> Result<(), ()> {
    let holder = holder(host, level, chunk(pos)).map_err(|_| ())?;
    consume(host, holder, resource)
}

fn consume(host: &mut dyn ServerHost, holder: Holder, resource: Resource) -> Result<(), ()> {
    let mut saved = read(host, holder, resource).map_err(|_| ())?.ok_or(())?;
    saved.remaining = saved.remaining.checked_sub(1).ok_or(())?;
    resource.deposit().set(host, holder, saved).map_err(|_| ())
}

pub(crate) fn restore(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    entries: &[(Resource, u32)],
) -> Result<Vec<(Resource, u32)>, &'static str> {
    let holder = holder(host, level, chunk(pos))?;
    restore_entries(host, holder, entries)
}

fn restore_entries(
    host: &mut dyn ServerHost,
    holder: Holder,
    entries: &[(Resource, u32)],
) -> Result<Vec<(Resource, u32)>, &'static str> {
    host.transaction(|host, _| {
        let mut result = Vec::with_capacity(entries.len());
        for &(resource, imported) in entries {
            let remaining = if let Some(saved) = read(host, holder, resource)? {
                imported_remaining(saved, imported)?
            } else {
                resource
                    .deposit()
                    .set(
                        host,
                        holder,
                        Reserve {
                            remaining: imported,
                            paper_import: Some(imported),
                        },
                    )
                    .map_err(|_| "The Paper GEO reserve could not be saved.")?;
                imported
            };
            result.push((resource, remaining));
        }
        Ok(result)
    })
}

fn imported_remaining(previous: Reserve, imported: u32) -> Result<u32, &'static str> {
    match previous.paper_import {
        Some(original) if original == imported => Ok(previous.remaining),
        _ => Err("This chunk already has a different GEO reserve."),
    }
}

fn oil_supply(biome: &str) -> u32 {
    match biome {
        "minecraft:beach" | "minecraft:stony_shore" => 6,
        "minecraft:river" => 16,
        "minecraft:swamp" => 20,
        "minecraft:ice_spikes"
        | "minecraft:frozen_ocean"
        | "minecraft:frozen_river"
        | "minecraft:frozen_peaks"
        | "minecraft:snowy_slopes" => 24,
        "minecraft:badlands" | "minecraft:wooded_badlands" | "minecraft:eroded_badlands" => 40,
        "minecraft:desert" => 45,
        "minecraft:ocean"
        | "minecraft:cold_ocean"
        | "minecraft:warm_ocean"
        | "minecraft:lukewarm_ocean" => 64,
        "minecraft:deep_ocean" | "minecraft:deep_cold_ocean" | "minecraft:deep_lukewarm_ocean" => {
            72
        }
        "minecraft:windswept_hills"
        | "minecraft:windswept_gravelly_hills"
        | "minecraft:jagged_peaks" => 20,
        "minecraft:snowy_plains" | "minecraft:snowy_taiga" => 16,
        "minecraft:mushroom_fields" => 20,
        _ => 10,
    }
}

pub(crate) fn scan(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    rolls: &Rolls,
) -> Result<Vec<(Resource, u32)>, &'static str> {
    let [x, z] = chunk(pos);
    let holder = holder(host, level, [x, z])?;
    host.transaction(|host, _| {
        let mut sample = None;
        let mut result = Vec::with_capacity(SCANNER_RESOURCES.len());
        for resource in SCANNER_RESOURCES {
            let remaining = if let Some(saved) = read(host, holder, resource)? {
                saved.remaining
            } else {
                if sample.is_none() {
                    let biome = level
                        .biome_at(host, [x * 16, pos[1], z * 16])
                        .ok_or("The GEO biome sample is not loaded.")?;
                    let dimension = level
                        .dimension()
                        .id(host)
                        .ok_or("The GEO dimension is unknown.")?;
                    sample = Some((dimension, biome));
                }
                let (dimension, biome) = sample.as_ref().unwrap();
                let remaining = resource
                    .generate(dimension, biome, |bound| rolls.next(host, level, bound))
                    .ok_or("Could not sample a GEO reserve.")?;
                resource
                    .deposit()
                    .set(
                        host,
                        holder,
                        Reserve {
                            remaining,
                            paper_import: None,
                        },
                    )
                    .map_err(|_| "Could not save the GEO reserve.")?;
                remaining
            };
            result.push((resource, remaining));
        }
        Ok(result)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mining {
    Unscanned,
    Exhausted,
    Available(Resource),
}

pub(crate) fn mining(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> Result<Mining, &'static str> {
    let holder = holder(host, level, chunk(pos))?;
    choose(|resource| read(host, holder, resource).map(|saved| saved.map(|saved| saved.remaining)))
}

fn choose(
    mut supply: impl FnMut(Resource) -> Result<Option<u32>, &'static str>,
) -> Result<Mining, &'static str> {
    for resource in MINER_RESOURCES {
        match supply(resource)? {
            None => return Ok(Mining::Unscanned),
            Some(0) => {}
            Some(_) => return Ok(Mining::Available(resource)),
        }
    }
    Ok(Mining::Exhausted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_rolls_back_with_operation_and_multiresource_import_is_atomic() {
        let mut host = mod_api::testing::FakeHost::new();
        let holder = Holder::Chunk([-1, 2]);
        let salt = Resource::Salt;
        let uranium = Resource::Uranium;
        let ice = Resource::NetherIce;
        let initial = [(salt, 8), (uranium, 0)];
        assert_eq!(
            restore_entries(&mut host, holder, &initial),
            Ok(initial.to_vec())
        );
        let refused: Result<(), ()> = host.transaction(|host, _| {
            consume(host, holder, salt)?;
            Err(())
        });
        assert_eq!(refused, Err(()));
        assert_eq!(read(&mut host, holder, salt).unwrap().unwrap().remaining, 8);
        assert_eq!(consume(&mut host, holder, salt), Ok(()));
        assert_eq!(consume(&mut host, holder, uranium), Err(()));
        assert_eq!(
            restore_entries(&mut host, holder, &initial),
            Ok(vec![(salt, 7), (uranium, 0)])
        );
        assert!(restore_entries(&mut host, holder, &[(ice, 6), (salt, 5)]).is_err());
        assert!(read(&mut host, holder, ice).unwrap().is_none());
        assert_eq!(read(&mut host, holder, salt).unwrap().unwrap().remaining, 7);
    }

    #[test]
    fn resource_tables_follow_paper_and_one_sided_exclusive_deviation() {
        assert_eq!(chunk([-1, 80, -17]), [-1, -2]);
        let ow = "minecraft:overworld";
        let nether = "minecraft:the_nether";
        let end = "minecraft:the_end";
        for (resource, dimension, biome, base, bound) in [
            (Resource::Oil, ow, "minecraft:plains", 10, 8),
            (Resource::Oil, ow, "minecraft:stony_shore", 6, 8),
            (Resource::Oil, ow, "minecraft:snowy_taiga", 16, 8),
            (Resource::Oil, ow, "minecraft:jagged_peaks", 20, 8),
            (Resource::Oil, ow, "minecraft:frozen_ocean", 24, 8),
            (Resource::Oil, ow, "minecraft:badlands", 40, 8),
            (Resource::Oil, ow, "minecraft:desert", 45, 8),
            (Resource::Oil, ow, "minecraft:warm_ocean", 64, 8),
            (Resource::Oil, ow, "minecraft:deep_cold_ocean", 72, 8),
            (
                Resource::NetherIce,
                nether,
                "minecraft:nether_wastes",
                32,
                6,
            ),
            (
                Resource::NetherIce,
                nether,
                "minecraft:warped_forest",
                48,
                6,
            ),
            (
                Resource::NetherIce,
                nether,
                "minecraft:basalt_deltas",
                64,
                6,
            ),
            (Resource::Uranium, ow, "minecraft:plains", 4, 2),
            (Resource::Uranium, ow, "minecraft:beach", 5, 2),
            (Resource::Uranium, ow, "minecraft:stony_peaks", 8, 2),
            (Resource::Uranium, ow, "minecraft:dripstone_caves", 12, 2),
            (Resource::Salt, ow, "minecraft:plains", 6, 18),
            (Resource::Salt, nether, "minecraft:basalt_deltas", 8, 18),
            (Resource::Salt, ow, "minecraft:swamp", 20, 18),
            (Resource::Salt, ow, "minecraft:stony_shore", 40, 18),
            (Resource::Salt, ow, "minecraft:warm_ocean", 60, 18),
        ] {
            let values: Vec<_> = (0..bound)
                .map(|value| {
                    resource
                        .generate(dimension, biome, |actual| {
                            assert_eq!(actual, bound);
                            Some(value)
                        })
                        .unwrap()
                })
                .collect();
            assert_eq!(values, (base..base + bound).collect::<Vec<_>>());
        }
        for (resource, dimension) in [
            (Resource::Oil, nether),
            (Resource::Uranium, nether),
            (Resource::NetherIce, ow),
            (Resource::Salt, end),
            (Resource::Salt, "example:custom"),
        ] {
            assert_eq!(
                resource.generate(dimension, "minecraft:desert", |_| unreachable!()),
                Some(0)
            );
        }
    }

    #[test]
    fn paper_exhaustion_is_distinct_from_unscanned_and_reimport_cannot_refill() {
        for resource in SCANNER_RESOURCES {
            let exhausted = Reserve {
                remaining: 0,
                paper_import: Some(8),
            };
            let serialized = exhausted.encode(resource).unwrap();
            assert_eq!(
                serialized.get(&resource.paper_key().into()),
                Some(&Tag::Int(0))
            );
            assert_eq!(Reserve::decode(&serialized, resource), Ok(exhausted));
            assert_eq!(imported_remaining(exhausted, 8), Ok(0));
            assert!(imported_remaining(exhausted, 9).is_err());
            let generated = Reserve {
                remaining: 8,
                paper_import: None,
            };
            assert!(imported_remaining(generated, 8).is_err());
        }
        assert_eq!(OIL_DEPOSIT.name, "geo_oil");
        let legacy = Compound::from([("slimefun-oil".into(), Tag::Int(0))]);
        assert_eq!(
            Reserve::decode(&legacy, Resource::Oil).unwrap().remaining,
            0
        );
        assert_eq!(choose(|_| Ok(None)), Ok(Mining::Unscanned));
        assert_eq!(choose(|_| Ok(Some(0))), Ok(Mining::Exhausted));
        assert_eq!(
            choose(|_| Ok(Some(1))),
            Ok(Mining::Available(Resource::Uranium))
        );
        assert_eq!(
            choose(|resource| match resource {
                Resource::Uranium => Ok(Some(0)),
                Resource::NetherIce => Ok(Some(1)),
                _ => unreachable!(),
            }),
            Ok(Mining::Available(Resource::NetherIce)),
        );
    }
}
