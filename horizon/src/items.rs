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
