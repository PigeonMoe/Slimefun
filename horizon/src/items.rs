// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{ComponentId, Compound, Host, Identifier, ItemResource, ItemStack, Tag};

pub(crate) const ID_KEY: &str = "slimefun:slimefun_item";

pub(crate) fn paper_item_data(id: &str) -> Tag {
    Tag::compound([("PublicBukkitValues", Tag::compound([(ID_KEY, string(id))]))])
}

pub(crate) fn stack_limit(host: &dyn Host, stack: &ItemStack) -> u32 {
    let resource = ItemResource::of(stack);
    let key = ComponentId::new("minecraft:max_stack_size").expect("built-in item component");
    match resource.value(host, &key) {
        Some(Tag::Int(max @ 1..=99)) => max as u32,
        _ => host.max_stack_size(resource.item()).unwrap_or(1),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Vanilla(&'static str),
    Potion(&'static str),
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
    pub juice: Option<JuiceProfile>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct JuiceProfile {
    pub color: i32,
    pub effect: &'static str,
    pub duration: i32,
}

impl JuiceProfile {
    pub fn is_saturating(&self) -> bool {
        self.effect == "minecraft:saturation"
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HeadTexture {
    pub profile: [i32; 4],
    pub value: &'static str,
}

impl HeadTexture {
    pub(crate) fn component(&self) -> Tag {
        Tag::compound([
            ("id", Tag::IntArray(self.profile.into())),
            (
                "properties",
                Tag::List(vec![Tag::compound([
                    ("name", string("textures")),
                    ("value", string(self.value)),
                ])]),
            ),
        ])
    }
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Vanilla(id) => id,
            Self::Potion(_) => "minecraft:potion",
            Self::Slimefun(item) => item.id,
        }
    }

    pub(crate) const fn material(self) -> &'static str {
        match self {
            Self::Vanilla(id) => id,
            Self::Potion(_) => "minecraft:potion",
            Self::Slimefun(item) => item.material,
        }
    }

    pub fn juice(self) -> Option<&'static JuiceProfile> {
        match self {
            Self::Vanilla(_) | Self::Potion(_) => None,
            Self::Slimefun(item) => item.juice.as_ref(),
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
            Self::Potion(expected) => matches_potion(stack, expected),
            Self::Slimefun(item) => item_id(stack).as_deref() == Some(item.id),
        }
    }

    pub fn decorate(self, stack: &mut ItemStack) {
        if let Self::Slimefun(item) = self {
            let _ = stack.apply_components(&item.components());
        }
    }

    pub fn stack(self, host: &dyn Host, count: u32) -> Option<ItemStack> {
        self.stack_with(host, count, SlimefunItem::components)
    }

    pub(crate) fn guide_stack(
        self,
        host: &dyn Host,
        count: u32,
        language: crate::guide::Language,
    ) -> Option<ItemStack> {
        if language == crate::guide::Language::CHINESE {
            return self.stack(host, count);
        }
        self.stack_with(host, count, |item| {
            let mut components = item.base_components(false);
            language.localize_components(item.id, &mut components);
            components
        })
    }

    fn stack_with(
        self,
        host: &dyn Host,
        count: u32,
        components: impl FnOnce(&SlimefunItem) -> Compound,
    ) -> Option<ItemStack> {
        let material = Identifier::parse(self.material()).ok()?;
        let mut stack = ItemStack::new(host.item_key(&material)?, count)?;
        match self {
            Self::Slimefun(item) => stack.set_components(components(item)).ok()?,
            Self::Potion(potion) => stack
                .set_components(Compound::from([(
                    "minecraft:potion_contents".into(),
                    Tag::compound([("potion", string(potion))]),
                )]))
                .ok()?,
            Self::Vanilla(_) => {}
        }
        Some(stack)
    }
}

fn matches_potion(stack: &ItemStack, expected: &str) -> bool {
    let Some(components) = stack.components().map(|patch| patch.as_nbt()) else {
        return false;
    };
    let potion = components
        .get(&"minecraft:potion_contents".into())
        .and_then(|contents| match contents {
            Tag::String(_) => Some(contents),
            _ => contents.get("potion"),
        });
    let Some(Tag::String(potion)) = potion else {
        return false;
    };
    let Ok(potion) = potion.to_string() else {
        return false;
    };
    let has_lore = matches!(
        components.get(&"minecraft:lore".into()),
        Some(Tag::List(lines)) if !lines.is_empty()
    );
    let has_model = match components.get(&"minecraft:custom_model_data".into()) {
        Some(Tag::Int(_)) => true,
        Some(model) => matches!(model.get("floats"), Some(Tag::List(floats)) if !floats.is_empty()),
        None => false,
    };
    potion.trim_start_matches("minecraft:") == expected.trim_start_matches("minecraft:")
        && !has_lore
        && !has_model
        && !components.contains_key(&"minecraft:custom_name".into())
}

impl SlimefunItem {
    fn components(&self) -> Compound {
        let mut components = self.base_components(true);
        crate::guide::localize_item_components(self.id, &mut components);
        components
    }

    fn base_components(&self, legacy_name: bool) -> Compound {
        let name = legacy_name.then(|| crate::machine_lore::name(self.id)).flatten()
            .unwrap_or_else(|| text(self.name, self.color, false));
        let mut components = Compound::from([
            ("minecraft:custom_name".into(), name),
            ("minecraft:custom_data".into(), paper_item_data(self.id)),
        ]);
        if self.id == "HOLOGRAM_PROJECTOR" {
            components.insert("minecraft:lore".into(), Tag::List(vec![
                text("", "white", false), text("Projects an Editable Hologram", "white", false),
            ]));
        }
        if self.id == "PORTABLE_CRAFTER" {
            components.insert("minecraft:lore".into(), crate::portable_crafter::lore());
        }
        if self.id == "TAPE_MEASURE" {
            components.insert("minecraft:lore".into(), crate::tape_measure::lore());
        }
        if let Some(lore) = crate::infused::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::energy_gadgets::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::network_recipes::cargo_lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::machine_lore::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::thermal_machines::jerky_lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::android_items::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::factory::crop_growth::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if self.id == crate::gps_transport::PORTABLE_TELEPORTER.id() {
            components.insert(
                "minecraft:lore".into(),
                Tag::List(vec![
                    string(""),
                    text("This device allows you to teleport", "white", false),
                    text("to your waypoints from anywhere", "white", false),
                    string(""),
                    crate::factory::charging_bench::charge_lore(0.0, 50.0),
                    string(""),
                    text("Right Click to use", "yellow", false),
                ]),
            );
        }
        if self.id == crate::geology::GPS_EMERGENCY_TRANSMITTER.id() {
            components.insert("minecraft:lore".into(), crate::geology::emergency_lore());
        }
        if let Some(lore) = crate::factory::animal_machines::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::food_machines::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) =
            crate::composter::lore(self.id).or_else(|| crate::block_placer::lore(self.id))
        {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(tier) = self.tier {
            let line = format!("Tier: {tier}");
            components.insert(
                "minecraft:lore".into(),
                Tag::List(vec![string(""), text(&line, "red", true)]),
            );
        }
        if let Some(lore) = crate::backpacks::item_lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::network_runtime::trash::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::auto_anvil::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if let Some(lore) = crate::book_binder::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        crate::enhanced_furnaces::decorate(self.id, self.name, &mut components);
        if self.id == IGNITION_CHAMBER.id() {
            components.insert("minecraft:lore".into(), crate::ignition_chamber::lore());
        }
        if self.id == crate::factory::produce_collector::PRODUCE_COLLECTOR.id() {
            components.insert(
                "minecraft:lore".into(),
                crate::factory::produce_collector::lore(),
            );
        }
        crate::enchantment_machines::decorate(self.id, &mut components);
        crate::ancient_altar::decorate(self.id, &mut components);
        crate::soulbound_gear::decorate(self.id, &mut components);
        crate::rainbow::decorate(self.id, &mut components);
        crate::gps_transport::decorate(self.id, &mut components);
        crate::elevator::decorate(self.id, &mut components);
        crate::medical::decorate(self.id, &mut components);
        crate::magic_eye::decorate(self.id, &mut components);
        crate::climbing_pick::decorate(self.id, &mut components);
        crate::grappling_hook::decorate(self.id, &mut components);
        crate::multi_tool::decorate(self.id, &mut components);
        crate::knowledge_tome::decorate(self.id, &mut components);
        if self.id == GOLD_24K_BLOCK.id() {
            components.insert(
                "minecraft:custom_name".into(),
                crate::hologram_projector::colored_lore("&fGold Block &7(24-Carat)"),
            );
        }
        let component_lore: &[&str] = match self.id {
            "DEBUG_FISH" => crate::debug_fish::LORE,
            "FLASK_OF_KNOWLEDGE" => &[
                "",
                "&fAllows you to store some of",
                "&fyour Experience in a Bottle",
                "&7Cost: &a1 Level",
            ],
            "GOLD_PAN" => &["", "&eRight Click&7 to collect resources", "&7from Gravel"],
            "NETHER_GOLD_PAN" => &["", "&eRight Click&7 to collect resources", "&7from Soul Sand"],
            "SOLAR_PANEL" => &["", "&7Important component for", "&7crafting a &bSolar Generator"],
            "CARGO_MOTOR" => &[
                "",
                "&7Important ingredient for items",
                "&7related to Cargo Management",
            ],
            "COPPER_WIRE" => &["", "&6Crucial component in electric modules"],
            "SYNTHETIC_DIAMOND" => &[
                "",
                "&fThis item can be used in a",
                "&fworkbench and acts like a normal Diamond",
            ],
            "SYNTHETIC_EMERALD" => &["", "&fThis item can be used to", "&ftrade with Villagers"],
            "MAGNESIUM_SALT" => &[
                "",
                "&7A special type of fuel that can be",
                "&7used in a Magnesium-powered Generator",
            ],
            "GPS_TRANSMITTER" => &["", "&8⇨ &e⚡ &716 J Buffer", "&8⇨ &e⚡ &72 J/s"],
            "GPS_TRANSMITTER_2" => &["", "&8⇨ &e⚡ &764 J Buffer", "&8⇨ &e⚡ &76 J/s"],
            "GPS_TRANSMITTER_3" => &["", "&8⇨ &e⚡ &7256 J Buffer", "&8⇨ &e⚡ &722 J/s"],
            "GPS_TRANSMITTER_4" => &["", "&8⇨ &e⚡ &71024 J Buffer", "&8⇨ &e⚡ &792 J/s"],
            "GPS_MARKER_TOOL" => &[
                "",
                "&fAllows you to set a Waypoint at",
                "&fthe Location you place this",
            ],
            "GPS_CONTROL_PANEL" => &[
                "",
                "&fAllows you to track your Satellites",
                "&fand manage your Waypoints",
            ],
            "ANDROID_INTERFACE_FUEL" => &[
                "",
                "&fItems stored in this Interface",
                "&fwill be inserted into an Android's Fuel Slot",
                "&fwhen its Script tells them to do so",
            ],
            "ANDROID_INTERFACE_ITEMS" => &[
                "",
                "&fItems stored in an Android's Inventory",
                "&fwill be inserted into this Interface",
                "&fwhen its Script tells them to do so",
            ],
            "GPS_GEO_SCANNER" => &["", "&fScans a Chunk for natural Resources", "&fsuch as &8Oil"],
            "PORTABLE_GEO_SCANNER" => &[
                "",
                "&fScans a Chunk for natural Resources",
                "",
                "&eRight Click&7 to scan",
            ],
            "GEO_MINER" => &[
                "",
                "&eMines up resources from the chunk",
                "&eThese Resources cannot be mined with a pickaxe",
                "",
                "&6Advanced Machine",
                "&8⇨ &b⚡ &7Speed: &b1.0x",
                "&8⇨ &e⚡ &748 J/s",
                "",
                "&c&l! &cMake sure to Geo-Scan the Chunk first",
            ],
            "OIL_PUMP" => &[
                "",
                "&7Pumps up Oil and fills it into Buckets",
                "",
                "&c&l! &cMake sure to Geo-Scan the Chunk first",
            ],
            "GPS_TELEPORTATION_MATRIX" => &[
                "",
                "&fThis is your Teleporter's Main Component",
                "&fThis Matrix allows Players to choose from all",
                "&fWaypoints made by the Player who has placed",
                "&fthis Device.",
            ],
            "GPS_ACTIVATION_DEVICE_SHARED" => &[
                "",
                "&fPlace this onto a Teleportation Matrix",
                "&fand step onto this Plate to activate",
                "&fthe Teleportation Process",
            ],
            "GPS_ACTIVATION_DEVICE_PERSONAL" => &[
                "",
                "&fPlace this onto a Teleportation Matrix",
                "&fand step onto this Plate to activate",
                "&fthe Teleportation Process",
                "",
                "&fThis Version only allows the Person who",
                "&fplaced this Device to use it",
            ],
            "SYNTHETIC_SHULKER_SHELL" => &[
                "",
                "&fThis item can be used in a",
                "&fworkbench like a normal Shulker Shell",
            ],
            "SYNTHETIC_SAPPHIRE" => &[
                "",
                "&fThis item can be used in a",
                "&fworkbench and acts like Lapis Lazuli",
            ],
            "CARBONADO" => &["", "&7&o\"Black Diamond\""],
            _ => &[],
        };
        if !component_lore.is_empty() {
            components.insert(
                "minecraft:lore".into(),
                Tag::List(
                    component_lore
                        .iter()
                        .copied()
                        .map(crate::hologram_projector::colored_lore)
                        .collect(),
                ),
            );
        }
        if let Some(lore) = crate::radiation::item_lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        crate::zombie_pills::decorate(self.id, &mut components);
        crate::nether_goo::decorate(self.id, &mut components);
        if self.id == crate::infernal_bonemeal::INFERNAL_BONEMEAL.id() {
            components.insert("minecraft:lore".into(), crate::infernal_bonemeal::lore());
        }
        crate::snacks::decorate(self.id, &mut components);
        crate::seasonal_gifts::decorate(self.id, &mut components);
        crate::smelters_pickaxe::decorate(self.id, &mut components);
        if self.id == crate::seismic_axe::SEISMIC_AXE.id() {
            components.insert("minecraft:lore".into(), crate::seismic_axe::lore());
        }
        if self.id == crate::lumber_axe::LUMBER_AXE.id() {
            components.insert("minecraft:lore".into(), crate::lumber_axe::lore());
        }
        crate::spawner_repair::decorate(self.id, &mut components);
        crate::magic_weapons::decorate(self.id, &mut components);
        if self.id == crate::beheading::SWORD_OF_BEHEADING.id() {
            components.insert("minecraft:lore".into(), crate::beheading::lore());
        }
        crate::ore_tools::decorate(self.id, &mut components);
        if let Some(lore) = crate::talismans::lore(self.id) {
            components.insert("minecraft:lore".into(), lore);
        }
        if self.id == crate::teleposition_scroll::SCROLL_OF_DIMENSIONAL_TELEPOSITION.id() {
            components.insert("minecraft:lore".into(), crate::teleposition_scroll::lore());
        }
        crate::explosive_tools::decorate(self.id, &mut components);
        crate::jet_boots::decorate(self.id, &mut components);
        crate::jetpack::decorate(self.id, &mut components);
        crate::elemental_staves::decorate(self.id, &mut components);
        crate::storm_staff::decorate(self.id, &mut components);
        crate::armor_forge::decorate(self.id, &mut components);
        crate::auto_crafter::decorate(self.id, &mut components);
        let enchantments: &[(&str, i32)] = match self.id {
            "GRANDMAS_WALKING_STICK" => &[("minecraft:knockback", 2)],
            "GRANDPAS_WALKING_STICK" => &[("minecraft:knockback", 5)],
            "COBALT_PICKAXE" => &[("minecraft:efficiency", 6), ("minecraft:unbreaking", 10)],
            _ => &[],
        };
        if !enchantments.is_empty() {
            components.insert(
                "minecraft:enchantments".into(),
                Tag::compound(enchantments.iter().map(|&(id, level)| (id, Tag::Int(level)))),
            );
        }
        if let Some(texture) = &self.texture {
            components.insert("minecraft:profile".into(), texture.component());
        }
        if let Some(juice) = &self.juice {
            components.insert(
                "minecraft:potion_contents".into(),
                Tag::compound([
                    ("custom_color", Tag::Int(juice.color)),
                    (
                        "custom_effects",
                        Tag::List(vec![Tag::compound([
                            ("id", string(juice.effect)),
                            ("duration", Tag::Int(juice.duration)),
                            ("ambient", Tag::Byte(1)),
                        ])]),
                    ),
                ]),
            );
            components.insert(
                "!minecraft:use_remainder".into(),
                Tag::Compound(Compound::new()),
            );
            if juice.is_saturating() {
                components.insert(
                    "minecraft:tooltip_display".into(),
                    Tag::compound([(
                        "hidden_components",
                        Tag::List(vec![string("minecraft:potion_contents")]),
                    )]),
                );
                components.insert(
                    "minecraft:lore".into(),
                    Tag::List(vec![
                        string(""),
                        Tag::compound([
                            ("text", string("恢复 ")),
                            ("color", string("gray")),
                            ("italic", Tag::Byte(1)),
                            (
                                "extra",
                                Tag::List(vec![
                                    text(
                                        &format!("{:.1}", (f64::from(juice.duration) + 1.0) / 2.0),
                                        "aqua",
                                        true,
                                    ),
                                    text(" 点饥饿值", "gray", true),
                                ]),
                            ),
                        ]),
                    ]),
                );
            }
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

pub(crate) fn usable_in_workbench(stack: &ItemStack) -> bool {
    item_id(stack).is_none_or(|id| {
        !crate::availability::visible(crate::ticker_policy::Tickers::current(), None, &id) || matches!(
            id.as_str(),
            "SYNTHETIC_SAPPHIRE"
                | "SYNTHETIC_DIAMOND"
                | "SYNTHETIC_EMERALD"
                | "GOLD_4K"
                | "SYNTHETIC_SHULKER_SHELL"
        )
    })
}

fn string(value: &str) -> Tag {
    Tag::String(value.into())
}

pub(crate) fn text(value: &str, color: &str, italic: bool) -> Tag {
    Tag::compound([
        ("text", string(value)),
        ("color", string(color)),
        ("italic", Tag::Byte(i8::from(italic))),
    ])
}

macro_rules! item {
    ($id:ident, $material:literal, $name:literal, $color:literal, $tier:expr, $texture:expr) => {
        pub(crate) const $id: $crate::items::Kind =
            $crate::items::Kind::Slimefun(&$crate::items::SlimefunItem {
                id: stringify!($id),
                material: concat!("minecraft:", $material),
                name: $name,
                color: $color,
                tier: $tier,
                texture: $texture,
                juice: None,
            });
    };
}
pub(crate) use item;

macro_rules! juice {
    ($id:ident, $name:literal, $text_color:literal, $color:literal) => {
        juice!($id, $name, $text_color, $color, "minecraft:saturation", 5);
    };
    ($id:ident, $name:literal, $text_color:literal, $color:literal, $effect:literal, $duration:literal) => {
        pub(crate) const $id: Kind = Kind::Slimefun(&SlimefunItem {
            id: stringify!($id),
            material: "minecraft:potion",
            name: $name,
            color: $text_color,
            tier: None,
            texture: None,
            juice: Some(JuiceProfile {
                color: $color,
                effect: $effect,
                duration: $duration,
            }),
        });
    };
}

juice!(
    CHRISTMAS_MILK,
    "Glass of Milk",
    "gold",
    0xffffff,
    "minecraft:saturation",
    4
);
juice!(
    CHRISTMAS_CHOCOLATE_MILK,
    "Chocolate Milk",
    "gold",
    0x800000,
    "minecraft:saturation",
    11
);
juice!(
    CHRISTMAS_EGG_NOG,
    "Egg Nog",
    "green",
    0x808080,
    "minecraft:saturation",
    6
);
juice!(
    CHRISTMAS_APPLE_CIDER,
    "Apple Cider",
    "red",
    0xff0000,
    "minecraft:saturation",
    13
);
juice!(
    CHRISTMAS_HOT_CHOCOLATE,
    "Hot Chocolate",
    "gold",
    0x800000,
    "minecraft:saturation",
    13
);

juice!(APPLE_JUICE, "Apple Juice", "red", 0xff0000);
juice!(CARROT_JUICE, "Carrot Juice", "gold", 0xffa500);
juice!(MELON_JUICE, "Melon Juice", "red", 0xff0000);
juice!(PUMPKIN_JUICE, "Pumpkin Juice", "gold", 0xffa500);
juice!(SWEET_BERRY_JUICE, "Sweet Berry Juice", "red", 0xff0000);
juice!(GLOW_BERRY_JUICE, "Glow Berry Juice", "gold", 0xffa500);
juice!(
    GOLDEN_APPLE_JUICE,
    "Golden Apple Juice",
    "aqua",
    0xffff00,
    "minecraft:absorption",
    400
);

item!(
    CHRISTMAS_FRUIT_CAKE,
    "pumpkin_pie",
    "Fruit Cake",
    "green",
    None,
    None
);
item!(
    CHRISTMAS_APPLE_PIE,
    "pumpkin_pie",
    "Apple Pie",
    "white",
    None,
    None
);
item!(
    CHRISTMAS_CAKE,
    "pumpkin_pie",
    "Christmas Cake",
    "green",
    None,
    None
);
item!(CHRISTMAS_CARAMEL, "brick", "Caramel", "gold", None, None);
item!(
    CHRISTMAS_CARAMEL_APPLE,
    "apple",
    "Caramel Apple",
    "gold",
    None,
    None
);
item!(
    CHRISTMAS_CHOCOLATE_APPLE,
    "apple",
    "Chocolate Apple",
    "gold",
    None,
    None
);
item!(CARROT_PIE, "pumpkin_pie", "Carrot Pie", "gold", None, None);
item!(
    EASTER_APPLE_PIE,
    "pumpkin_pie",
    "Apple Pie",
    "white",
    None,
    None
);

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

        let mut previous = lump.clone();
        previous
            .apply_components(&Compound::from([(
                "minecraft:custom_name".into(),
                text("Magical Lump", "yellow", false),
            )]))
            .unwrap();
        assert!(MAGIC_LUMP_1.matches(&host, &previous));

        let mut wrong_material = Kind::Vanilla("minecraft:sugar").stack(&host, 1).unwrap();
        wrong_material
            .set_components(lump.components().unwrap().as_nbt().clone())
            .unwrap();
        assert!(!MAGIC_LUMP_1.matches(&host, &wrong_material));
    }

    #[test]
    fn restored_paper_lore_is_preserved_and_localized() {
        use crate::hologram_projector::colored_lore;

        let cases = [
            (
                GOLD_PAN,
                &["", "&eRight Click&7 to collect resources", "&7from Gravel"][..],
                &["", "&e右键&7", "&7从沙砾中淘金"][..],
            ),
            (
                COPPER_WIRE,
                &["", "&6Crucial component in electric modules"][..],
                &["", "&6电子配件中的重要组成部分"][..],
            ),
            (
                crate::heated_pressure::TINY_URANIUM,
                &["", "&a☢&7 Radiation level: &eLOW"][..],
                &["", "&a☢&7 辐射等级: &e低"][..],
            ),
            (
                crate::network_recipes::BLISTERING_INGOT_3,
                &[
                    "",
                    "&a☢&7 Radiation level: &cVERY HIGH",
                    "&8⇨ &4Hazmat Suit required!",
                ][..],
                &["", "&a☢&7 辐射等级: &c极高", "&8⇨ &4需要穿戴防化服！"][..],
            ),
            (
                crate::android_items::PROGRAMMABLE_ANDROID_2_FARMER,
                &[
                    "",
                    "&8⇨ &7Function: Farming",
                    "&8⇨ &7Fuel Efficiency: 1.5x",
                    "&8⇨ &7Can also harvest Plants from ExoticGarden",
                ][..],
                &[
                    "",
                    "&8⇨ &7功能: 耕作",
                    "&8⇨ &7燃料效率: 1.5x",
                    "&8⇨ &7可以收割异域花园内的植物",
                ][..],
            ),
        ];
        for (kind, original, translated) in cases {
            let Kind::Slimefun(item) = kind else {
                panic!("{} is not a Slimefun item", kind.id());
            };
            for (components, lines) in [
                (item.base_components(false), original),
                (item.components(), translated),
            ] {
                let expected = Tag::List(lines.iter().copied().map(colored_lore).collect());
                assert_eq!(
                    components.get(&"minecraft:lore".into()),
                    Some(&expected),
                    "{}",
                    kind.id(),
                );
            }
        }
    }

    #[test]
    fn walking_sticks_and_cobalt_pick_keep_paper_enchantments_and_research() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:stick", "minecraft:iron_pickaxe"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let equipment: [(Kind, &str, &[(&str, i32)]); 3] = [
            (GRANDMAS_WALKING_STICK, "walking_sticks", &[("minecraft:knockback", 2)]),
            (GRANDPAS_WALKING_STICK, "walking_sticks", &[("minecraft:knockback", 5)]),
            (COBALT_PICKAXE, "cobalt_pickaxe", &[("minecraft:efficiency", 6), ("minecraft:unbreaking", 10)]),
        ];
        for (kind, study, enchantments) in equipment {
            let stack = kind.stack(&host, 1).unwrap();
            assert_eq!(item_id(&stack).as_deref(), Some(kind.id()));
            let recipe = crate::recipes::crafting()
                .find(|recipe| recipe.output.kind == kind)
                .unwrap();
            assert_eq!((recipe.output.count, recipe.research.id), (1, study));
            let patch = stack.components().unwrap().as_nbt();
            let expected = Tag::compound(enchantments.iter().map(|&(key, level)| (key, Tag::Int(level))));
            assert_eq!(
                patch.get(&"minecraft:enchantments".into()),
                Some(&expected)
            );
        }
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

item!(
    GOLD_24K_BLOCK,
    "gold_block",
    "Gold Block (24-Carat)",
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
    GRANDMAS_WALKING_STICK,
    "stick",
    "Grandmas Walking Stick",
    "gray",
    None,
    None
);
item!(
    GRANDPAS_WALKING_STICK,
    "stick",
    "Grandpas Walking Stick",
    "gray",
    None,
    None
);
item!(COBALT_PICKAXE, "iron_pickaxe", "Cobalt Pickaxe", "blue", None, None);
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
    NETHER_GOLD_PAN,
    "bowl",
    "Nether Gold Pan",
    "dark_red",
    None,
    None
);
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

item!(
    SYNTHETIC_DIAMOND,
    "diamond",
    "Synthetic Diamond",
    "aqua",
    None,
    None
);
item!(
    RAW_CARBONADO,
    "player_head",
    "Raw Carbonado",
    "aqua",
    None,
    Some(HeadTexture {
        profile: [-86148934, 3223659, -1993303417, -1687965865],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2ViNDllNmVjMTA3NzFlODk5MjI1YWVhNzNjZDhjZjAzNjg0ZjQxMWQxNDE1YzczMjNjOTNjYjk0NzYyMzAifX19",
    })
);
item!(
    CARBONADO,
    "player_head",
    "Carbonado",
    "aqua",
    None,
    Some(HeadTexture {
        profile: [-680051172, 1087714401, -1329937131, -1657014872],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzEyZjRiMTU3N2Y1MTYwYzY4OTMxNzI1NzFjNGE3MWQ4YjMyMWNkY2VhYTAzMmM2ZTBlM2I2MGUwYjMyOGZhIn19fQ==",
    })
);

item!(
    BASIC_CIRCUIT_BOARD,
    "activator_rail",
    "Basic Circuit Board",
    "aqua",
    None,
    None
);
item!(
    ADVANCED_CIRCUIT_BOARD,
    "powered_rail",
    "Advanced Circuit Board",
    "aqua",
    None,
    None
);
item!(
    IGNITION_CHAMBER,
    "dropper",
    "Automatic Ignition Chamber",
    "dark_red",
    None,
    None
);
item!(
    BATTERY,
    "player_head",
    "Battery",
    "gold",
    None,
    Some(HeadTexture {
        profile: [1506400309, -904645575, -1715297863, 98820824],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzZlMmRkYTZlZjYxODVkNGRkNmVhODY4NGU5N2QzOWJhOGFiMDM3ZTI1Zjc1Y2RlYTZiZDI5ZGY4ZWIzNGVlIn19fQ==",
    })
);
item!(
    MAGNET,
    "player_head",
    "Magnet",
    "red",
    None,
    Some(HeadTexture {
        profile: [-1689013977, 1490433317, -1663768500, 1586340172],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2FiYThlYmM0YzZhODE3MzA5NDc0OTliZjdlMWQ1ZTczZmVkNmMxYmIyYzA1MWU5NmQzNWViMTZkMjQ2MTBlNyJ9fX0=",
    })
);
item!(
    ELECTRO_MAGNET,
    "player_head",
    "Electromagnet",
    "red",
    None,
    Some(HeadTexture {
        profile: [-1689013977, 1490433317, -1663768500, 1586340172],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2FiYThlYmM0YzZhODE3MzA5NDc0OTliZjdlMWQ1ZTczZmVkNmMxYmIyYzA1MWU5NmQzNWViMTZkMjQ2MTBlNyJ9fX0=",
    })
);
item!(
    ELECTRIC_MOTOR,
    "player_head",
    "Electric Motor",
    "red",
    None,
    Some(HeadTexture {
        profile: [1236088620, -867814837, -1523178510, 245211413],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzhjYmNhMDEyZjY3ZTU0ZGU5YWVlNzJmZjQyNGUwNTZjMmFlNThkZTVlYWNjOTQ5YWIyYmNkOTY4M2NlYyJ9fX0=",
    })
);
item!(COPPER_WIRE, "string", "Copper Wire", "gold", None, None);
item!(
    SOLAR_PANEL,
    "daylight_detector",
    "Photovoltaic Cell",
    "blue",
    None,
    None
);
item!(
    REDSTONE_ALLOY,
    "brick",
    "Redstone Alloy Ingot",
    "red",
    None,
    None
);
item!(
    ENERGY_REGULATOR,
    "player_head",
    "Energy Regulator",
    "gold",
    None,
    Some(HeadTexture {
        profile: [-546435397, 1399734098, -1494264352, -475146256],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2Q3OGYyYjdlNWU3NTYzOWVhN2ZiNzk2YzM1ZDM2NGM0ZGYyOGI0MjQzZTY2Yjc2Mjc3YWFkY2Q2MjYxMzM3In19fQ==",
    })
);
item!(
    ENERGY_CONNECTOR,
    "player_head",
    "Energy Connector",
    "yellow",
    None,
    Some(HeadTexture {
        profile: [1779154149, 1612987419, -1733160209, 134025994],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzEwODVlMDk4NzU2Yjk5NWIwMDI0MTY0NDA4OWM1NWE4ZjlhY2RlMzViOWEzNzc4NWQ1ZTA1N2E5MjM2MTNiIn19fQ==",
    })
);
item!(
    SMALL_CAPACITOR,
    "player_head",
    "Small Energy Capacitor",
    "green",
    None,
    Some(HeadTexture {
        profile: [1570095643, -1019004906, -1651193732, -1212268975],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkxMzYxZTU3NmI0OTNjYmZkZmFlMzI4NjYxY2VkZDFhZGQ1NWZhYjRlNWViNDE4YjkyY2ViZjYyNzVmOGJiNCJ9fX0=",
    })
);
item!(
    SOLAR_GENERATOR,
    "daylight_detector",
    "Solar Generator",
    "aqua",
    None,
    None
);

pub(crate) fn decorative_head(id: &str) -> Option<(Kind, &'static crate::recipes::Research)> {
    use crate::{network_recipes, recipes};

    [
        (network_recipes::POWER_CRYSTAL, &recipes::POWER_ITEMS),
        (STONE_CHUNK, &recipes::ORE_WASHER),
        (
            network_recipes::SYNTHETIC_SAPPHIRE,
            &network_recipes::SYNTHETIC_SAPPHIRE_RESEARCH,
        ),
        (CARBONADO, &recipes::BLACK_DIAMONDS),
        (RAW_CARBONADO, &recipes::BLACK_DIAMONDS),
    ]
    .into_iter()
    .find(|(kind, _)| kind.id() == id)
}

pub(crate) fn network_item(id: &str) -> Option<Kind> {
    [
        GOLD_24K_BLOCK,
        crate::block_placer::BLOCK_PLACER,
        crate::knowledge_tome::TOME_OF_KNOWLEDGE_SHARING,
        crate::infused::INFUSED_HOPPER,
        ENERGY_REGULATOR,
        ENERGY_CONNECTOR,
        SMALL_CAPACITOR,
        SOLAR_GENERATOR,
        IGNITION_CHAMBER,
        crate::food_machines::CHEESE,
        crate::food_machines::BUTTER,
        crate::factory::produce_collector::PRODUCE_COLLECTOR,
        crate::factory::tree_growth::TREE_GROWTH_ACCELERATOR,
        crate::factory::assemblers::IRON_GOLEM_ASSEMBLER,
        crate::factory::assemblers::WITHER_ASSEMBLER,
        crate::advanced_processing::CRUCIBLE,
        crate::composter::COMPOSTER,
        crate::auto_anvil::DUCT_TAPE,
        crate::enchantment_machines::WITHER_PROOF_OBSIDIAN,
        crate::enchantment_machines::WITHER_PROOF_GLASS,
        crate::network_recipes::HARDENED_GLASS,
        ELECTRIC_FURNACE,
        ELECTRIC_FURNACE_2,
        ELECTRIC_FURNACE_3,
        ELECTRIC_ORE_GRINDER,
        ELECTRIC_ORE_GRINDER_2,
        ELECTRIC_ORE_GRINDER_3,
        ELECTRIC_INGOT_PULVERIZER,
    ]
    .into_iter()
    .find(|kind| kind.id() == id)
    .or_else(|| decorative_head(id).map(|(kind, _)| kind))
    .or_else(|| crate::network_recipes::network_item(id))
    .or_else(|| crate::auto_crafter::item_by_id(id))
    .or_else(|| crate::armor_forge::registered_item(id))
    .or_else(|| crate::heated_pressure::network_item(id))
    .or_else(|| crate::electric_processing::machine(id).map(|spec| spec.item))
    .or_else(|| crate::advanced_processing::machine(id).map(|spec| spec.item))
    .or_else(|| crate::enhanced_furnaces::by_id(id).map(|furnace| furnace.item))
    .or_else(|| crate::thermal_machines::item(id))
    .or_else(|| crate::geology::network_item(id))
    .or_else(|| crate::gps_transport::item(id))
    .or_else(|| crate::android_items::item(id))
    .or_else(|| crate::factory::crop_growth::machine(id).map(|machine| machine.item))
    .or_else(|| crate::factory::animal_machines::machine(id).map(|machine| machine.item))
    .or_else(|| {
        use crate::factory::experience_collector::{
            FILLED_FLASK_OF_KNOWLEDGE, FLASK_OF_KNOWLEDGE, XP_COLLECTOR,
        };
        [XP_COLLECTOR, FLASK_OF_KNOWLEDGE, FILLED_FLASK_OF_KNOWLEDGE]
            .into_iter()
            .find(|kind| kind.id() == id)
    })
    .or_else(|| crate::factory::reactors::reactor(id).map(|spec| spec.item))
    .or_else(|| {
        use crate::factory::reactors::{NEPTUNIUM, REACTOR_ACCESS_PORT};
        [NEPTUNIUM, REACTOR_ACCESS_PORT]
            .into_iter()
            .find(|kind| kind.id() == id)
    })
    .or_else(|| {
        use crate::network_runtime::trash::{PORTABLE_DUSTBIN, TRASH_CAN_BLOCK};
        [PORTABLE_DUSTBIN, TRASH_CAN_BLOCK]
            .into_iter()
            .find(|kind| kind.id() == id)
    })
    .or_else(|| {
        let pump = crate::factory::fluid_pump::FLUID_PUMP;
        (pump.id() == id).then_some(pump)
    })
    .or_else(|| crate::rainbow::item(id))
    .or_else(|| {
        let rune = crate::soulbound_rune::ANCIENT_RUNE_SOULBOUND;
        (rune.id() == id).then_some(rune)
    })
    .or_else(|| crate::fuel_generators::generator(id).map(|spec| spec.item))
    .or_else(|| {
        [
            crate::fuel_generators::BUCKET_OF_FUEL,
            crate::fuel_generators::STRANGE_NETHER_GOO,
        ]
        .into_iter()
        .find(|item| item.id() == id)
    })
}

item!(
    HEATING_COIL,
    "player_head",
    "Heating Coil",
    "red",
    None,
    Some(HeadTexture {
        profile: [493332113, 165100908, -1818174556, -1968188076],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzdlM2JjNDg5M2JhNDFhM2Y3M2VlMjgxNzRjZGY0ZmVmNmIxNDVlNDFmZTZjODJjYjdiZThkOGU5NzcxYTUifX19"
    })
);
item!(GILDED_IRON, "gold_ingot", "Gilded Iron", "gold", None, None);
item!(
    ELECTRIC_FURNACE,
    "furnace",
    "Electric Furnace",
    "red",
    None,
    None
);
item!(
    ELECTRIC_FURNACE_2,
    "furnace",
    "Electric Furnace - II",
    "red",
    None,
    None
);
item!(
    ELECTRIC_FURNACE_3,
    "furnace",
    "Electric Furnace - III",
    "red",
    None,
    None
);

item!(
    ELECTRIC_ORE_GRINDER,
    "furnace",
    "Electric Ore Grinder",
    "red",
    None,
    None
);

item!(
    ELECTRIC_ORE_GRINDER_2,
    "furnace",
    "Electric Ore Grinder (II)",
    "red",
    None,
    None
);

item!(
    ELECTRIC_ORE_GRINDER_3,
    "furnace",
    "Electric Ore Grinder (III)",
    "red",
    None,
    None
);

item!(
    ELECTRIC_INGOT_PULVERIZER,
    "furnace",
    "Electric Ingot Pulverizer",
    "red",
    None,
    None
);
