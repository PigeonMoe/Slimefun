// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    AttachmentCodec, AttachmentType, BlockEntityRef, ComponentId, Compound, Hand, Holder, Host,
    ITEMS, ItemPlace, ItemResource, ItemStack, LevelRef, MenuReplace, MenuSlotPlace, PlayerRef,
    Registrar, ResourceHandler, ServerHost, SoundSource, Tag, Text, Transaction,
};
use std::collections::BTreeMap;

use crate::{
    items::{ALUMINUM_INGOT, ELECTRIC_MOTOR, HeadTexture, Kind, LEAD_INGOT, item},
    network_recipes::CARGO_MOTOR,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

use super::{Position, block_data, cargo_topology::item_key, read_stack};

const TEXTURE: HeadTexture = HeadTexture {
    profile: [-548511929, 1847409297, -1959609336, -25343845],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzMyZDQxMDQyY2U5OTE0N2NjMzhjYWM5ZTQ2NzQxNTc2ZTdlZTc5MTI4M2U2ZmFjOGQzMjkyY2FlMjkzNWYxZiJ9fX0=",
};
item!(
    TRASH_CAN_BLOCK,
    "player_head",
    "Trash Can",
    "dark_aqua",
    None,
    Some(TEXTURE)
);
item!(
    PORTABLE_DUSTBIN,
    "player_head",
    "Portable Dustbin",
    "gold",
    None,
    Some(TEXTURE)
);

pub(crate) const CAN_RESEARCH: Research = Research {
    id: "trash_can",
    name: "Trash",
    cost: 8,
};
pub(crate) const PORTABLE_RESEARCH: Research = Research {
    id: "portable_dustbin",
    name: "Portable Dustbin",
    cost: 2,
};
pub(crate) const RESEARCHES: &[Research] = &[CAN_RESEARCH, PORTABLE_RESEARCH];

const IRON: Option<Kind> = Some(Kind::Vanilla("minecraft:iron_ingot"));
pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [IRON, IRON, IRON, IRON, None, IRON, IRON, IRON, IRON],
        output: Stack {
            kind: PORTABLE_DUSTBIN,
            count: 1,
        },
        research: &PORTABLE_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(PORTABLE_DUSTBIN),
            None,
            Some(LEAD_INGOT),
            Some(CARGO_MOTOR),
            Some(LEAD_INGOT),
            Some(ALUMINUM_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(ALUMINUM_INGOT),
        ],
        output: Stack {
            kind: TRASH_CAN_BLOCK,
            count: 1,
        },
        research: &CAN_RESEARCH,
    },
];

const SLOTS: [&str; 7] = ["10", "11", "12", "13", "14", "15", "16"];
const RESTORED: &str = "paper_restored";
static CONTENTS: AttachmentType<Compound> = AttachmentType {
    namespace: "slimefun",
    name: "trash_contents",
    default: Compound::new,
    codec: Some(AttachmentCodec {
        encode: |data| Ok(data.clone()),
        decode: |data| Ok(data.clone()),
    }),
    copy_on_death: false,
    sync: false,
};

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let white = |text: &str| {
        Tag::compound([
            ("text", Tag::String(text.into())),
            ("color", Tag::String("white".into())),
            ("italic", Tag::Byte(0)),
        ])
    };
    let empty = Tag::String("".into());
    let lines = match id {
        "TRASH_CAN_BLOCK" => vec![empty, white("Will destroy all Items put into it")],
        "PORTABLE_DUSTBIN" => vec![
            white("Your portable Item-Destroyer"),
            empty,
            Tag::compound([
                ("text", Tag::String("Right Click".into())),
                ("color", Tag::String("yellow".into())),
                ("italic", Tag::Byte(0)),
                ("extra", Tag::List(vec![Tag::compound([
                    ("text", Tag::String(" to open".into())),
                    ("color", Tag::String("gray".into())),
                ])])),
            ]),
        ],
        _ => return None,
    };
    Some(Tag::List(lines))
}

pub(crate) fn portable(host: &mut dyn ServerHost, player: PlayerRef, hand: Hand) -> bool {
    let place = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    };
    if !place
        .stack(host)
        .is_some_and(|stack| PORTABLE_DUSTBIN.matches(host, &stack))
    {
        return false;
    }
    if check_research(host, player, PORTABLE_RESEARCH) {
        let mut title = Text::literal("Delete Items");
        title.style.color = Some("dark_red".into());
        if matches!(
            host.open_menu(player.uuid(), 3, title, vec![], MenuReplace::InPlace),
            Ok(Some(_))
        ) {
            player.play_sound(
                host,
                "minecraft:block.anvil.land",
                SoundSource::Master,
                1.0,
                1.0,
            );
        }
    }
    true
}

fn check_research(host: &mut dyn ServerHost, player: PlayerRef, requirement: Research) -> bool {
    if research::can_use(host, player, &requirement) {
        return true;
    }
    let _ = player.tell(
        host,
        Text::literal(format!("Research {} first.", requirement.name)),
    );
    false
}

pub(super) fn open(host: &mut dyn ServerHost, player: PlayerRef, level: LevelRef, pos: Position) {
    if !check_research(host, player, CAN_RESEARCH) {
        return;
    }
    let Some(entity) = host.block_entity(level, pos) else {
        return;
    };
    let backing = TrashInventory { level, pos, entity };
    let decoration = Kind::Vanilla("minecraft:red_stained_glass_pane")
        .stack(host, 1)
        .and_then(|mut stack| {
            stack
                .apply_components(&Compound::from([(
                    "minecraft:custom_name".into(),
                    Tag::String(" ".into()),
                )]))
                .ok()?;
            Some(stack)
        });
    let mut items = vec![decoration; 27];
    let mut places = vec![MenuSlotPlace::Decoration; 27];
    for (index, raw) in (10..=16).enumerate() {
        items[raw] = backing.stack(host, index);
        places[raw] = MenuSlotPlace::BlockEntityItems {
            entity,
            slot: index,
            allow_insert: true,
        };
    }
    if let Ok(Some(window)) = host.open_block_inventory_menu(
        player,
        entity,
        3,
        Text::literal("Trash Can"),
        items,
        MenuReplace::InPlace,
    ) && player.back_mod_menu_slots(host, window, places).is_err()
    {
        host.close_menu(player.uuid());
    }
}

pub(super) fn registered(host: &mut dyn ServerHost, level: LevelRef, pos: Position) -> bool {
    block_data(host, level, pos).is_some_and(|data| data.id == TRASH_CAN_BLOCK.id())
}

pub(super) fn register(registrar: &mut dyn Registrar) {
    for material in ["minecraft:player_head", "minecraft:player_wall_head"] {
        let key = registrar.block_key(material);
        registrar.block_capability_before_game(&ITEMS, &key, |host, at, _, entity| {
            let entity = entity?;
            (at.side.is_none() && registered(host, at.level, at.pos)).then_some(Box::new(
                TrashInventory {
                    level: at.level,
                    pos: at.pos,
                    entity,
                },
            ))
        });
    }
}

pub(super) fn clear(host: &mut dyn ServerHost, pos: Position) {
    let holder = Holder::BlockEntity(pos);
    if let Ok(Some(mut data)) = CONTENTS.get_existing(host, holder) {
        let old_len = data.len();
        for slot in SLOTS {
            data.remove(&slot.into());
        }
        if data.len() != old_len {
            let _ = CONTENTS.set(host, holder, data);
        }
    }
}

pub(super) fn restore(
    host: &mut dyn ServerHost,
    pos: Position,
    row: &toml::Table,
) -> Result<(), String> {
    if !row.contains_key("paper_inventory") {
        return Ok(());
    }
    let holder = Holder::BlockEntity(pos);
    let existing = CONTENTS
        .get(host, holder)
        .map_err(|error| format!("Could not read the trash can inventory ({error:?})."))?;
    if matches!(existing.get(&RESTORED.into()), Some(Tag::Byte(1))) {
        return Ok(());
    }
    crate::storage::validate_inventory_preset(row, TRASH_CAN_BLOCK.id())?;
    let items = crate::storage::decode_inventory(host, row)?.unwrap_or_default();
    let data = import_slots(host, existing, items)?;
    CONTENTS
        .set(host, holder, data)
        .map_err(|error| format!("Could not save the trash can inventory ({error:?})."))
}

fn import_slots(
    host: &dyn Host,
    mut data: Compound,
    items: BTreeMap<u32, ItemStack>,
) -> Result<Compound, String> {
    for (slot, item) in items {
        let raw = slot.to_string();
        if !SLOTS.contains(&raw.as_str()) {
            return Err(format!("Unexpected Paper trash can inventory slot {slot}."));
        }
        if data.contains_key(&raw.as_str().into()) {
            return Err("Trash can inventory already contains items in that slot.".into());
        }
        data.insert(
            raw.as_str().into(),
            Tag::Compound(item.to_nbt(host).ok_or("Unknown archived item.")?),
        );
    }
    data.insert(RESTORED.into(), Tag::Byte(1));
    Ok(data)
}

struct TrashInventory {
    level: LevelRef,
    pos: Position,
    entity: BlockEntityRef,
}

impl TrashInventory {
    fn data(&self, host: &mut dyn ServerHost) -> Option<Compound> {
        if host.block_entity(self.level, self.pos) != Some(self.entity)
            || !registered(host, self.level, self.pos)
        {
            return None;
        }
        CONTENTS.get(host, Holder::BlockEntity(self.pos)).ok()
    }

    fn stack(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemStack> {
        let data = self.data(host)?;
        let Tag::Compound(stack) = data.get(&(*SLOTS.get(index)?).into())? else {
            return None;
        };
        read_stack(stack, |id| item_key(host, id))
    }

    fn write(
        &self,
        host: &mut dyn ServerHost,
        index: usize,
        stack: Option<ItemStack>,
    ) -> Option<()> {
        let slot = *SLOTS.get(index)?;
        let mut data = self.data(host)?;
        if let Some(stack) = stack {
            data.insert(slot.into(), Tag::Compound(stack.to_nbt(host)?));
        } else {
            data.remove(&slot.into());
        }
        CONTENTS.set(host, Holder::BlockEntity(self.pos), data).ok()
    }
}

impl ResourceHandler<ItemResource> for TrashInventory {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        SLOTS.len()
    }

    fn resource(&self, host: &mut dyn ServerHost, slot: usize) -> Option<ItemResource> {
        self.stack(host, slot).as_ref().map(ItemResource::of)
    }

    fn amount(&self, host: &mut dyn ServerHost, slot: usize) -> u32 {
        self.stack(host, slot).as_ref().map_or(0, ItemStack::count)
    }

    fn capacity(&self, host: &mut dyn ServerHost, slot: usize, item: &ItemResource) -> u32 {
        if slot >= SLOTS.len() {
            return 0;
        }
        let max_stack = ComponentId::new("minecraft:max_stack_size").expect("built-in component");
        match item.value(host, &max_stack) {
            Some(Tag::Int(size @ 1..=99)) => size.min(64) as u32,
            _ => host.max_stack_size(item.item()).unwrap_or(0).min(64),
        }
    }

    fn insert(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        slot: usize,
        item: &ItemResource,
        amount: u32,
    ) -> u32 {
        let previous = self.stack(host, slot);
        if previous
            .as_ref()
            .is_some_and(|stack| ItemResource::of(stack) != *item)
        {
            return 0;
        }
        let count = previous.as_ref().map_or(0, ItemStack::count);
        let accepted = self
            .capacity(host, slot, item)
            .saturating_sub(count)
            .min(amount);
        if accepted > 0
            && let Some(stack) = item.to_stack(count + accepted)
            && self.write(host, slot, Some(stack)).is_some()
        {
            return accepted;
        }
        0
    }

    fn extract(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        slot: usize,
        item: &ItemResource,
        amount: u32,
    ) -> u32 {
        let Some(stack) = self
            .stack(host, slot)
            .filter(|stack| ItemResource::of(stack) == *item)
        else {
            return 0;
        };
        let accepted = stack.count().min(amount);
        if accepted > 0 && self.write(host, slot, stack.shrink(accepted)).is_some() {
            return accepted;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{HostTransaction, Identifier, prelude::BlockEntitiesHost, testing::FakeHost};

    #[test]
    fn trash_voids_items_on_tick_and_keeps_its_paper_restore_marker() {
        let mut host = FakeHost::new();
        let pos = [2, 64, 5];
        let level = host.level.level();
        host.level.block_entity(pos);
        let entity = host.block_entity(level, pos).unwrap();
        let (_, state) = super::super::imported(TRASH_CAN_BLOCK.id(), &toml::Table::new()).unwrap();
        super::super::write_data(&mut host, pos, state).unwrap();
        let archive: toml::Table = "inventory_preset = 'TRASH_CAN_BLOCK'\npaper_inventory = []\npaper_items = []\ninventory_slots = 0".parse().unwrap();
        restore(&mut host, pos, &archive).unwrap();

        host.registries
            .resolve_items([Identifier::parse("minecraft:stone").unwrap()])
            .unwrap();
        let stack = Kind::Vanilla("minecraft:stone").stack(&host, 1).unwrap();
        let resource = ItemResource::of(&stack);
        let inventory = TrashInventory { level, pos, entity };
        assert!(
            host.transaction(|host, tx| {
                assert_eq!(inventory.insert(host, tx, 0, &resource, 1), 1);
                Err::<(), _>(())
            })
            .is_err()
        );
        assert_eq!(inventory.amount(&mut host, 0), 0);

        host.transaction(|host, tx| {
            assert_eq!(inventory.insert(host, tx, 0, &resource, 1), 1);
            Ok::<(), ()>(())
        })
        .unwrap();
        assert_eq!(inventory.amount(&mut host, 0), 1);
        clear(&mut host, pos);
        assert_eq!(inventory.amount(&mut host, 0), 0);
        assert!(matches!(
            inventory.data(&mut host).unwrap().get(&RESTORED.into()),
            Some(Tag::Byte(1))
        ));
    }

    #[test]
    fn paper_trash_restores_only_the_seven_input_slots_and_does_not_refill_after_voiding() {
        let mut host = FakeHost::new();
        let pos = [2, 64, 5];
        let level = host.level.level();
        host.level.block_entity(pos);
        let entity = host.block_entity(level, pos).unwrap();
        let holder = Holder::BlockEntity(pos);
        let (_, state) = super::super::imported(TRASH_CAN_BLOCK.id(), &toml::Table::new()).unwrap();
        super::super::write_data(&mut host, pos, state).unwrap();
        host.registries
            .resolve_items(
                ["minecraft:stone", "minecraft:diamond_sword"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let stone = Kind::Vanilla("minecraft:stone").stack(&host, 13).unwrap();
        let mut sword = Kind::Vanilla("minecraft:diamond_sword")
            .stack(&host, 1)
            .unwrap();
        sword
            .apply_components(&Compound::from([
                ("minecraft:damage".into(), Tag::Int(12)),
                (
                    "minecraft:custom_data".into(),
                    Tag::compound([(
                        "PublicBukkitValues",
                        Tag::compound([(
                            "migration_fixture:original",
                            Tag::String("Paper trash".into()),
                        )]),
                    )]),
                ),
            ]))
            .unwrap();

        let inventory = TrashInventory { level, pos, entity };
        for illegal in [9, 17] {
            assert_eq!(
                import_slots(
                    &host,
                    Compound::new(),
                    BTreeMap::from([(10, stone.clone()), (illegal, sword.clone())])
                )
                .unwrap_err(),
                format!("Unexpected Paper trash can inventory slot {illegal}.")
            );
            assert!(inventory.stack(&mut host, 0).is_none());
        }
        let occupied = Compound::from([("16".into(), Tag::Compound(stone.to_nbt(&host).unwrap()))]);
        assert!(
            import_slots(
                &host,
                occupied,
                BTreeMap::from([(10, stone.clone()), (16, sword.clone())])
            )
            .unwrap_err()
            .contains("already contains items")
        );

        let prepared = import_slots(
            &host,
            Compound::new(),
            BTreeMap::from([(10, stone.clone()), (16, sword.clone())]),
        )
        .unwrap();
        CONTENTS
            .set(&mut host.attachments, holder, prepared)
            .unwrap();
        let saved = host.attachments.to_nbt();
        host.attachments = mod_api::Attachments::from_nbt(saved);
        assert_eq!(inventory.stack(&mut host, 0), Some(stone));
        assert_eq!(inventory.stack(&mut host, 6), Some(sword));
        assert!((1..6).all(|slot| inventory.stack(&mut host, slot).is_none()));

        clear(&mut host, pos);
        let archive: toml::Table = r#"
            paper_inventory = "original snapshot"
            inventory_preset = "TRASH_CAN_BLOCK"
            inventory_slots = 2
            paper_items = [
                {slot=10, id="minecraft:stone", count=13, data_version=4903, components={}},
                {slot=16, id="minecraft:diamond_sword", count=1, data_version=4903, components={"minecraft:damage" = "12", "minecraft:custom_data" = '{PublicBukkitValues:{"migration_fixture:original":"Paper trash"}}'}},
            ]
        "#
        .parse()
        .unwrap();
        restore(&mut host, pos, &archive).unwrap();
        assert!((0..7).all(|slot| inventory.stack(&mut host, slot).is_none()));
        assert_eq!(
            CONTENTS.get(&mut host.attachments, holder).unwrap(),
            Compound::from([(RESTORED.into(), Tag::Byte(1))])
        );
    }
}
