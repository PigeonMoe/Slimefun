// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, Host, Identifier, ItemStack, Tag};

pub(crate) const ID_KEY: &str = "slimefun:slimefun_item";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Vanilla(&'static str),
    Slimefun(&'static SlimefunItem),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SlimefunItem {
    pub id: &'static str,
    pub material: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    pub tier: Option<&'static str>,
    pub texture: Option<HeadTexture>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HeadTexture {
    pub profile: [i32; 4],
    pub value: &'static str,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Vanilla(id) => id,
            Self::Slimefun(item) => item.id,
        }
    }

    fn material(self) -> &'static str {
        match self {
            Self::Vanilla(id) => id,
            Self::Slimefun(item) => item.material,
        }
    }

    pub fn matches(self, host: &dyn Host, stack: &ItemStack) -> bool {
        let Some(material) = Identifier::parse(self.material()).ok() else {
            return false;
        };
        if host.item_key(&material) != Some(stack.item()) {
            return false;
        }
        match self {
            Self::Vanilla(_) => stack.components().is_none() && stack.damage_component().is_none(),
            Self::Slimefun(item) => item_id(stack).as_deref() == Some(item.id),
        }
    }

    pub fn decorate(self, stack: &mut ItemStack) {
        if let Self::Slimefun(item) = self {
            let _ = stack.apply_components(&item.components());
        }
    }

    pub fn stack(self, host: &dyn Host, count: u32) -> Option<ItemStack> {
        let material = Identifier::parse(self.material()).ok()?;
        let mut stack = ItemStack::new(host.item_key(&material)?, count)?;
        if let Self::Slimefun(item) = self {
            stack.set_components(item.components()).ok()?;
        }
        Some(stack)
    }
}

impl SlimefunItem {
    fn components(&self) -> Compound {
        let mut components = Compound::from([
            (
                "minecraft:custom_name".into(),
                text(self.name, self.color, false),
            ),
            (
                "minecraft:custom_data".into(),
                Tag::compound([(
                    "PublicBukkitValues",
                    Tag::compound([(ID_KEY, string(self.id))]),
                )]),
            ),
        ]);
        if let Some(tier) = self.tier {
            let line = format!("Tier: {tier}");
            components.insert(
                "minecraft:lore".into(),
                Tag::List(vec![string(""), text(&line, "red", true)]),
            );
        }
        if let Some(texture) = &self.texture {
            components.insert(
                "minecraft:profile".into(),
                Tag::compound([
                    ("id", Tag::IntArray(texture.profile.into())),
                    (
                        "properties",
                        Tag::List(vec![Tag::compound([
                            ("name", string("textures")),
                            ("value", string(texture.value)),
                        ])]),
                    ),
                ]),
            );
        }
        components
    }
}

pub(crate) fn item_id(stack: &ItemStack) -> Option<String> {
    let patch = stack.components()?.as_nbt();
    match patch
        .get(&"minecraft:custom_data".into())?
        .get("PublicBukkitValues")?
        .get(ID_KEY)?
    {
        Tag::String(id) => id.to_string().ok(),
        _ => None,
    }
}

fn string(value: &str) -> Tag {
    Tag::String(value.into())
}

fn text(value: &str, color: &str, italic: bool) -> Tag {
    Tag::compound([
        ("text", string(value)),
        ("color", string(color)),
        ("italic", Tag::Byte(i8::from(italic))),
    ])
}

macro_rules! item {
    ($id:ident, $material:literal, $name:literal, $color:literal, $tier:expr, $texture:expr) => {
        pub(crate) const $id: Kind = Kind::Slimefun(&SlimefunItem {
            id: stringify!($id),
            material: concat!("minecraft:", $material),
            name: $name,
            color: $color,
            tier: $tier,
            texture: $texture,
        });
    };
}

item!(
    MAGIC_LUMP_1,
    "gold_nugget",
    "Magical Lump - I",
    "gold",
    Some("I"),
    None
);
item!(
    MAGIC_LUMP_2,
    "gold_nugget",
    "Magical Lump - II",
    "gold",
    Some("II"),
    None
);
item!(
    MAGIC_LUMP_3,
    "gold_nugget",
    "Magical Lump - III",
    "gold",
    Some("III"),
    None
);
item!(
    ENDER_LUMP_1,
    "gold_nugget",
    "Ender Lump - I",
    "dark_purple",
    Some("I"),
    None
);
item!(
    ENDER_LUMP_2,
    "gold_nugget",
    "Ender Lump - II",
    "dark_purple",
    Some("II"),
    None
);
item!(
    ENDER_LUMP_3,
    "gold_nugget",
    "Ender Lump - III",
    "dark_purple",
    Some("III"),
    None
);
item!(WHEAT_FLOUR, "sugar", "Wheat Flour", "white", None, None);
item!(
    KELP_COOKIE,
    "cookie",
    "Kelp Cookie",
    "dark_green",
    None,
    None
);

// Profiles from the corresponding HeadTexture entries in the Paper branch.
item!(
    STONE_CHUNK,
    "player_head",
    "Stone Chunk",
    "gold",
    None,
    Some(HeadTexture {
        profile: [1888474698, -1924581807, -1723709629, 255373869],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2NlOGY1YWRiMTRkNmM5ZjZiODEwZDAyNzU0M2YxYThjMWY0MTdlMmZlZDk5M2M5N2JjZDg5Yzc0ZjVlMmU4In19fQ==",
    })
);
item!(
    CARBON,
    "player_head",
    "Carbon",
    "yellow",
    None,
    Some(HeadTexture {
        profile: [-1723066978, -1092009993, -1844264993, 142917949],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzhiM2EwOTViNmI4MWU2Yjk4NTNhMTkzMjRlZWRmMGJiOTM0OTQxNzI1OGRkMTczYjhlZmY4N2EwODdhYSJ9fX0=",
    })
);

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::testing::FakeHost;

    #[test]
    fn lump_matching_uses_material_and_paper_item_id() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:gold_nugget", "minecraft:sugar"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let vanilla = Kind::Vanilla("minecraft:gold_nugget")
            .stack(&host, 1)
            .unwrap();
        let lump = MAGIC_LUMP_1.stack(&host, 2).unwrap();
        let ender = ENDER_LUMP_1.stack(&host, 2).unwrap();

        assert_eq!(item_id(&lump).as_deref(), Some("MAGIC_LUMP_1"));
        assert!(MAGIC_LUMP_1.matches(&host, &lump));
        assert!(!MAGIC_LUMP_1.matches(&host, &vanilla));
        assert!(!MAGIC_LUMP_1.matches(&host, &ender));
        assert!(!Kind::Vanilla("minecraft:gold_nugget").matches(&host, &lump));

        let mut wrong_material = Kind::Vanilla("minecraft:sugar").stack(&host, 1).unwrap();
        wrong_material
            .set_components(lump.components().unwrap().as_nbt().clone())
            .unwrap();
        assert!(!MAGIC_LUMP_1.matches(&host, &wrong_material));
    }
}

// Resource items used by the starter production machines.
item!(
    COMPRESSED_CARBON,
    "player_head",
    "Compressed Carbon",
    "red",
    None,
    Some(HeadTexture {
        profile: [-1629223925, -1804255912, -1790465149, 1989611940],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzMyMWQ0OTUxNjU3NDhkMzExNmY5OWQ2YjViZDVkNDJlYjhiYTU5MmJjZGZhZDM3ZmQ5NWY5YjZjMDRhM2IifX19",
    })
);
item!(
    CARBON_CHUNK,
    "player_head",
    "Carbon Chunk",
    "dark_red",
    None,
    Some(HeadTexture {
        profile: [-1629223925, -1804255912, -1790465149, 1989611940],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzMyMWQ0OTUxNjU3NDhkMzExNmY5OWQ2YjViZDVkNDJlYjhiYTU5MmJjZGZhZDM3ZmQ5NWY5YjZjMDRhM2IifX19",
    })
);
item!(STEEL_PLATE, "paper", "Steel Plate", "gray", None, None);
item!(
    REINFORCED_PLATE,
    "paper",
    "Reinforced Plate",
    "gray",
    None,
    None
);
item!(STEEL_INGOT, "iron_ingot", "Steel Ingot", "aqua", None, None);
item!(
    REINFORCED_ALLOY_INGOT,
    "iron_ingot",
    "Reinforced Alloy Ingot",
    "aqua",
    None,
    None
);
item!(IRON_DUST, "gunpowder", "Iron Dust", "gold", None, None);
item!(GOLD_DUST, "glowstone_dust", "Gold Dust", "gold", None, None);
item!(
    COPPER_DUST,
    "glowstone_dust",
    "Copper Dust",
    "gold",
    None,
    None
);
item!(SULFATE, "glowstone_dust", "Sulfate", "gold", None, None);
item!(SIFTED_ORE, "gunpowder", "Sifted Ore", "gold", None, None);
item!(CRUSHED_ORE, "gunpowder", "Crushed Ore", "gold", None, None);
item!(
    PULVERIZED_ORE,
    "gunpowder",
    "Pulverized Ore",
    "gold",
    None,
    None
);
item!(
    GOLD_4K,
    "gold_ingot",
    "Gold Ingot (4-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_6K,
    "gold_ingot",
    "Gold Ingot (6-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_8K,
    "gold_ingot",
    "Gold Ingot (8-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_10K,
    "gold_ingot",
    "Gold Ingot (10-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_12K,
    "gold_ingot",
    "Gold Ingot (12-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_14K,
    "gold_ingot",
    "Gold Ingot (14-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_16K,
    "gold_ingot",
    "Gold Ingot (16-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_18K,
    "gold_ingot",
    "Gold Ingot (18-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_20K,
    "gold_ingot",
    "Gold Ingot (20-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_22K,
    "gold_ingot",
    "Gold Ingot (22-Carat)",
    "white",
    None,
    None
);
item!(
    GOLD_24K,
    "gold_ingot",
    "Gold Ingot (24-Carat)",
    "white",
    None,
    None
);

item!(LEAD_INGOT, "iron_ingot", "Lead Ingot", "aqua", None, None);
item!(
    OUTPUT_CHEST,
    "chest",
    "Output Chest",
    "dark_red",
    None,
    None
);

// Starter ore-processing and alloy items.
item!(
    ALUMINUM_BRASS_INGOT,
    "gold_ingot",
    "Aluminum Brass Ingot",
    "aqua",
    None,
    None
);
item!(
    ALUMINUM_BRONZE_INGOT,
    "gold_ingot",
    "Aluminum Bronze Ingot",
    "aqua",
    None,
    None
);
item!(ALUMINUM_DUST, "sugar", "Aluminum Dust", "gold", None, None);
item!(
    ALUMINUM_INGOT,
    "iron_ingot",
    "Aluminum Ingot",
    "aqua",
    None,
    None
);
item!(
    BILLON_INGOT,
    "iron_ingot",
    "Billon Ingot",
    "aqua",
    None,
    None
);
item!(BRASS_INGOT, "gold_ingot", "Brass Ingot", "aqua", None, None);
item!(BRONZE_INGOT, "brick", "Bronze Ingot", "aqua", None, None);
item!(
    COBALT_INGOT,
    "iron_ingot",
    "Cobalt Ingot",
    "blue",
    None,
    None
);
item!(COPPER_INGOT, "brick", "Copper Ingot", "aqua", None, None);
item!(
    CORINTHIAN_BRONZE_INGOT,
    "gold_ingot",
    "Corinthian Bronze Ingot",
    "aqua",
    None,
    None
);
item!(
    DAMASCUS_STEEL_INGOT,
    "iron_ingot",
    "Damascus Steel Ingot",
    "aqua",
    None,
    None
);
item!(
    DURALUMIN_INGOT,
    "iron_ingot",
    "Duralumin Ingot",
    "aqua",
    None,
    None
);
item!(
    FERROSILICON,
    "iron_ingot",
    "Ferrosilicon",
    "aqua",
    None,
    None
);
item!(GOLD_PAN, "bowl", "Gold Pan", "gold", None, None);
item!(
    HARDENED_METAL_INGOT,
    "iron_ingot",
    "Hardened Metal",
    "aqua",
    None,
    None
);
item!(LEAD_DUST, "gunpowder", "Lead Dust", "gold", None, None);
item!(MAGNESIUM_DUST, "sugar", "Magnesium", "gold", None, None);
item!(
    MAGNESIUM_INGOT,
    "iron_ingot",
    "Magnesium Ingot",
    "aqua",
    None,
    None
);
item!(
    NICKEL_INGOT,
    "iron_ingot",
    "Nickel Ingot",
    "aqua",
    None,
    None
);
item!(
    PURE_ORE_CLUSTER,
    "gunpowder",
    "Pure Ore Cluster",
    "gold",
    None,
    None
);
item!(SALT, "sugar", "Salt", "white", None, None);
item!(SILICON, "firework_star", "Silicon", "gold", None, None);
item!(SILVER_DUST, "sugar", "Silver Dust", "gold", None, None);
item!(
    SILVER_INGOT,
    "iron_ingot",
    "Silver Ingot",
    "aqua",
    None,
    None
);
item!(
    SOLDER_INGOT,
    "iron_ingot",
    "Solder Ingot",
    "aqua",
    None,
    None
);
item!(TIN_DUST, "sugar", "Tin Dust", "gold", None, None);
item!(TIN_INGOT, "iron_ingot", "Tin Ingot", "aqua", None, None);
item!(ZINC_DUST, "sugar", "Zinc Dust", "gold", None, None);
item!(ZINC_INGOT, "iron_ingot", "Zinc Ingot", "aqua", None, None);
