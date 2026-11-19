// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, Tag};

use crate::{
    armor_forge::ANCIENT_RUNE_RAINBOW,
    items::{Kind, SlimefunItem, item, text},
    recipes::{CraftingRecipe, Research, Stack},
};

#[path = "rainbow_placed.rs"]
pub(crate) mod placed;

pub(crate) const RESEARCH: Research = Research {
    id: "rainbow_blocks",
    name: "Rainbow Blocks",
    cost: 24,
};
const UNRESTRICTED: Research = Research {
    id: "",
    name: "",
    cost: 0,
};

item!(
    CHRISTMAS_COOKIE,
    "cookie",
    "Christmas Cookie",
    "green",
    None,
    None
);

pub(crate) const COOKIE_RECIPE: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(Kind::Vanilla("minecraft:cookie")),
        Some(Kind::Vanilla("minecraft:sugar")),
        Some(Kind::Vanilla("minecraft:lime_dye")),
        None,
        None,
        None,
        None,
        None,
        None,
    ],
    output: Stack {
        kind: CHRISTMAS_COOKIE,
        count: 16,
    },
    research: &UNRESTRICTED,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Palette {
    All,
    Christmas,
    Valentine,
    Halloween,
}

impl Palette {
    pub(super) const fn colors(self) -> &'static [&'static str] {
        match self {
            Self::All => &[
                "white",
                "orange",
                "magenta",
                "light_blue",
                "yellow",
                "lime",
                "pink",
                "gray",
                "light_gray",
                "cyan",
                "purple",
                "blue",
                "brown",
                "green",
                "red",
                "black",
            ],
            Self::Christmas => &["red", "green"],
            Self::Valentine => &["magenta", "pink"],
            Self::Halloween => &["orange", "black"],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Rainbow {
    pub kind: Kind,
    pub suffix: &'static str,
    pub white: &'static str,
    pub palette: Palette,
}

impl Rainbow {
    pub fn accepts(self, block: &str) -> bool {
        if block == self.kind.material() {
            return true;
        }
        let Some(color) = block
            .strip_prefix("minecraft:")
            .and_then(|block| block.strip_suffix(self.suffix))
            .and_then(|prefix| prefix.strip_suffix('_'))
        else {
            return false;
        };
        self.palette.colors().contains(&color)
    }

    pub fn material(self, color: &str) -> String {
        format!("minecraft:{color}_{}", self.suffix)
    }
}

macro_rules! rainbow {
    ($id:expr, $name:expr, $suffix:literal, $base:literal, $palette:ident) => {
        Rainbow {
            kind: Kind::Slimefun(&SlimefunItem {
                id: $id,
                material: concat!("minecraft:", $base, "_", $suffix),
                name: $name,
                color: "dark_purple",
                tier: None,
                texture: None,
                juice: None,
            }),
            suffix: $suffix,
            white: concat!("minecraft:white_", $suffix),
            palette: Palette::$palette,
        }
    };
}

macro_rules! family {
    ($id:literal, $name:literal, $suffix:literal) => {
        [
            rainbow!($id, $name, $suffix, "white", All),
            rainbow!(
                concat!($id, "_XMAS"),
                concat!($name, " (Christmas)"),
                $suffix,
                "white",
                Christmas
            ),
            rainbow!(
                concat!($id, "_VALENTINE"),
                concat!($name, " (Valentine's Day)"),
                $suffix,
                "pink",
                Valentine
            ),
            rainbow!(
                concat!($id, "_HALLOWEEN"),
                concat!($name, " (Halloween)"),
                $suffix,
                "orange",
                Halloween
            ),
        ]
    };
}

const BLOCKS: [[Rainbow; 4]; 6] = [
    family!("RAINBOW_WOOL", "Rainbow Wool", "wool"),
    family!("RAINBOW_GLASS", "Rainbow Glass", "stained_glass"),
    family!(
        "RAINBOW_GLASS_PANE",
        "Rainbow Glass Pane",
        "stained_glass_pane"
    ),
    family!("RAINBOW_CLAY", "Rainbow Clay", "terracotta"),
    family!("RAINBOW_CONCRETE", "Rainbow Concrete", "concrete"),
    family!(
        "RAINBOW_GLAZED_TERRACOTTA",
        "Rainbow Glazed Terracotta",
        "glazed_terracotta"
    ),
];

pub(super) const BLOCK_COUNT: usize = BLOCKS.len() * BLOCKS[0].len();

pub(super) fn block(id: &str) -> Option<(usize, Rainbow)> {
    BLOCKS
        .iter()
        .flatten()
        .copied()
        .enumerate()
        .find(|(_, block)| block.kind.id() == id)
}

pub(crate) fn register(registrar: &mut dyn mod_api::Registrar) {
    for family in &BLOCKS {
        for index in [0, 2, 3] {
            let key = registrar.item_key(family[index].kind.material());
            registrar.item_part(key, placed::Placement);
        }
    }
}

pub(crate) fn item(id: &str) -> Option<Kind> {
    if id == CHRISTMAS_COOKIE.id() {
        Some(CHRISTMAS_COOKIE)
    } else {
        block(id).map(|(_, rainbow)| rainbow.kind)
    }
}

pub(super) fn is_carrier(id: &str) -> bool {
    BLOCKS.iter().any(|family| family[0].accepts(id))
}

const fn recipe(block: Rainbow) -> CraftingRecipe {
    let white = Some(Kind::Vanilla(block.white));
    let rune = Some(ANCIENT_RUNE_RAINBOW);
    let (first, middle, third) = match block.palette {
        Palette::All => (white, white, white),
        Palette::Christmas => (
            Some(Kind::Vanilla("minecraft:red_dye")),
            Some(CHRISTMAS_COOKIE),
            Some(Kind::Vanilla("minecraft:green_dye")),
        ),
        Palette::Valentine => (
            Some(Kind::Vanilla("minecraft:red_dye")),
            Some(Kind::Vanilla("minecraft:poppy")),
            Some(Kind::Vanilla("minecraft:pink_dye")),
        ),
        Palette::Halloween => (
            Some(Kind::Vanilla("minecraft:orange_dye")),
            Some(Kind::Vanilla("minecraft:pumpkin")),
            Some(Kind::Vanilla("minecraft:black_dye")),
        ),
    };
    let (count, research) = if matches!(block.palette, Palette::All) {
        (8, &RESEARCH)
    } else {
        (2, &UNRESTRICTED)
    };
    CraftingRecipe {
        inputs: [
            first, middle, third, white, rune, white, third, middle, first,
        ],
        output: Stack {
            kind: block.kind,
            count,
        },
        research,
    }
}

const fn altar_recipes() -> [CraftingRecipe; BLOCK_COUNT] {
    let mut recipes = [recipe(BLOCKS[0][0]); BLOCK_COUNT];
    let mut family = 0;
    let mut variant = 0;
    while family < BLOCKS.len() {
        recipes[family * BLOCKS[0].len() + variant] = recipe(BLOCKS[family][variant]);
        variant += 1;
        if variant == BLOCKS[0].len() {
            variant = 0;
            family += 1;
        }
    }
    recipes
}

pub(crate) const ALTAR_RECIPES: &[CraftingRecipe] = &altar_recipes();

pub(crate) fn decorate(id: &str, patch: &mut Compound) {
    if id == CHRISTMAS_COOKIE.id() {
        patch.insert(
            "minecraft:custom_name".into(),
            christmas("Christmas Cookie"),
        );
        return;
    }
    let Some((_, record)) = block(id) else {
        return;
    };
    let line = match record.palette {
        Palette::All => text(
            "Cycles through all Colors of the Rainbow!",
            "light_purple",
            false,
        ),
        Palette::Christmas => christmas("[Christmas Edition]"),
        Palette::Valentine => edition("Valentine's Day Edition", "dark_purple", "light_purple"),
        Palette::Halloween => edition("Halloween Edition", "red", "gold"),
    };
    patch.insert(
        "minecraft:lore".into(),
        Tag::List(vec![Tag::String("".into()), line]),
    );
    if record.palette != Palette::All
        && let Some((name, variant)) = match record.kind {
            Kind::Slimefun(item) => item.name.split_once(" ("),
            Kind::Vanilla(_) => None,
        }
    {
        patch.insert(
            "minecraft:custom_name".into(),
            component([
                text(name, "dark_purple", false),
                text(&format!(" ({variant}"), "gray", false),
            ]),
        );
    }
}

fn component(parts: impl IntoIterator<Item = Tag>) -> Tag {
    Tag::compound([
        ("text", Tag::String("".into())),
        ("italic", Tag::Byte(0)),
        ("extra", Tag::List(parts.into_iter().collect())),
    ])
}

fn edition(name: &str, brackets: &str, color: &str) -> Tag {
    component([
        text("[", brackets, false),
        text(name, color, false),
        text("]", brackets, false),
    ])
}

fn christmas(name: &str) -> Tag {
    component(name.chars().enumerate().map(|(index, letter)| {
        text(
            &letter.to_string(),
            if index % 2 == 0 { "green" } else { "red" },
            false,
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paper_rune_is_shared_with_rainbow_armor_and_preserves_its_exact_ring() {
        use crate::armor_forge::{ALTAR_RECIPES, ANCIENT_RUNE_ENDER};
        let rune = ALTAR_RECIPES
            .iter()
            .find(|recipe| recipe.output.kind == ANCIENT_RUNE_RAINBOW)
            .unwrap();
        assert_eq!(rune.output.count, 1);
        assert_eq!(
            (rune.research.id, rune.research.cost),
            ("special_runes", 18)
        );
        assert_eq!(
            rune.inputs,
            [
                Kind::Vanilla("minecraft:red_dye"),
                crate::items::MAGIC_LUMP_3,
                Kind::Vanilla("minecraft:cyan_dye"),
                Kind::Vanilla("minecraft:white_wool"),
                ANCIENT_RUNE_ENDER,
                Kind::Vanilla("minecraft:white_wool"),
                Kind::Vanilla("minecraft:yellow_dye"),
                crate::items::ENDER_LUMP_3,
                Kind::Vanilla("minecraft:magenta_dye"),
            ]
            .map(Some)
        );
    }

    #[test]
    fn all_paper_rainbow_variants_keep_their_material_and_exact_altar_matrix() {
        assert_eq!(ALTAR_RECIPES.len(), 24);
        for recipe in ALTAR_RECIPES {
            let (_, result) = block(recipe.output.kind.id()).unwrap();
            assert!(result.accepts(result.kind.material()));
            assert_eq!(recipe.inputs[4], Some(ANCIENT_RUNE_RAINBOW));
            assert_eq!(recipe.inputs[3], Some(Kind::Vanilla(result.white)));
            assert_eq!(recipe.inputs[5], recipe.inputs[3]);
            assert_eq!(recipe.inputs[6], recipe.inputs[2]);
            assert_eq!(recipe.inputs[7], recipe.inputs[1]);
            assert_eq!(recipe.inputs[8], recipe.inputs[0]);
            if result.palette == Palette::All {
                assert_eq!(recipe.output.count, 8);
                assert_eq!(recipe.research.id, RESEARCH.id);
                assert_eq!(recipe.inputs[0], recipe.inputs[3]);
                assert_eq!(recipe.inputs[1], recipe.inputs[3]);
                assert_eq!(recipe.inputs[2], recipe.inputs[3]);
            } else {
                assert_eq!(recipe.output.count, 2);
                assert!(recipe.research.id.is_empty());
            }
        }
        for (palette, suffix, ingredient) in [
            (Palette::Christmas, "XMAS", CHRISTMAS_COOKIE),
            (
                Palette::Valentine,
                "VALENTINE",
                Kind::Vanilla("minecraft:poppy"),
            ),
            (
                Palette::Halloween,
                "HALLOWEEN",
                Kind::Vanilla("minecraft:pumpkin"),
            ),
        ] {
            let (index, record) = block(&format!("RAINBOW_WOOL_{suffix}")).unwrap();
            assert_eq!(record.palette, palette);
            assert_eq!(ALTAR_RECIPES[index].inputs[1], Some(ingredient));
        }
        assert_eq!(COOKIE_RECIPE.output.count, 16);
        assert!(COOKIE_RECIPE.research.id.is_empty());
    }
}
