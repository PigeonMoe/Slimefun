// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use mod_api::{
    __private::game::BackendRefOf,
    AttachmentStore, BackendRef, BlockEntityRef, BlockKey, BlockState, BlockStateRecord,
    ConfigEntry, ConfigFileError, ConfigFiles, ConfigStat, DimensionKey, Host, Identifier, ItemKey,
    ModEntry, Registrar, RegistrationContext, TransactionSeal,
    prelude::{
        BlockEntitiesHost, ConfigServerHost, ItemsHost, ItemsServerHost, KeysBackend,
        LevelsBackend, ModStateHost, ModStateServerHost,
    },
    testing::FakeHost,
};
use std::sync::Arc;

const POSITION: [i32; 3] = [770, 112, 132];
const CARRIER: &str = "minecraft:brown_terracotta";

// FakeHost supplies transactions and attachments but lacks live dimension and
// block names. Give it the registry facts and normalized stack for this fixture.
struct OrphanHost(FakeHost, FixtureWorld);

struct FixtureWorld {
    carrier: BlockKey,
    material: &'static str,
    files: Mutex<BTreeMap<String, Vec<u8>>>,
}

struct CarrierFixture(&'static str, Arc<Mutex<Option<BlockKey>>>);

#[mod_api::__subscriptions]
impl mod_api::Hooks for CarrierFixture {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        *self.1.lock().unwrap() = Some(registrar.block_key(self.0));
    }
}

impl Host for OrphanHost {
    fn backend(&self) -> Option<BackendRef<'_>> {
        Some(BackendRef::of(self))
    }
}
impl ServerHost for OrphanHost {}
mod_api::impl_host_areas!(OrphanHost, except(block_entities, items, mod_state));
mod_api::impl_server_host_areas!(OrphanHost, except(config, items, mod_state));
mod_api::impl_backend_areas!(OrphanHost, except(levels, keys));
impl mod_api::Backend for OrphanHost {}

impl LevelsBackend for OrphanHost {
    fn dimension_key_id(&self, key: DimensionKey) -> Option<String> {
        (key == self.0.level.level().dimension()).then(|| "minecraft:overworld".into())
    }

    fn level_in_place(&self, level: LevelRef) -> bool {
        self.0.level_in_place(level)
    }

    fn level_block(&self, level: LevelRef, pos: [i32; 3]) -> Option<BlockKey> {
        (self.level_in_place(level) && pos == POSITION).then_some(self.1.carrier)
    }

    fn level_block_state(&self, level: LevelRef, pos: [i32; 3]) -> Option<BlockState> {
        Some(BlockStateRecord::new(self.level_block(level, pos)?, 0).into())
    }
}

impl KeysBackend for OrphanHost {
    fn block_key_id(&self, key: BlockKey) -> String {
        assert_eq!(self.1.carrier, key);
        self.1.material.into()
    }

    fn item_key_id(&self, key: ItemKey) -> String {
        self.0.item_key_id(key)
    }
}

impl BlockEntitiesHost for OrphanHost {
    fn block_entity(&self, level: LevelRef, pos: [i32; 3]) -> Option<BlockEntityRef> {
        self.0.block_entity(level, pos)
    }
}

impl ItemsHost for OrphanHost {
    fn item_key(&self, id: &Identifier) -> Option<ItemKey> {
        self.0.item_key(id)
    }

    fn max_stack_size(&self, item: ItemKey) -> Option<u32> {
        self.0.max_stack_size(item)
    }

    fn item_default_component(&self, item: ItemKey, component: &str) -> Option<Tag> {
        self.0.item_default_component(item, component)
    }
}

impl ItemsServerHost for OrphanHost {
    fn item_from_saved_components(
        &self,
        version: i32,
        item: &str,
        count: u32,
        components: Vec<(String, String)>,
    ) -> Option<ItemStack> {
        assert_eq!(
            (version, item, count, components.len()),
            (4903, "minecraft:gravel", 1, 0)
        );
        ItemStack::new(self.item_key(&Identifier::parse(item).ok()?)?, count)
    }
}

impl ModStateHost for OrphanHost {
    fn attachments(&mut self) -> &mut dyn AttachmentStore {
        self.0.attachments()
    }
}

impl ModStateServerHost for OrphanHost {
    fn transaction_open(&mut self) -> TransactionSeal {
        self.0.transaction_open()
    }

    fn transaction_close(&mut self, seal: &TransactionSeal, commit: bool) {
        self.0.transaction_close(seal, commit);
    }
}

impl ConfigServerHost for OrphanHost {
    fn world_data_files(&self) -> &dyn ConfigFiles {
        self
    }
}

impl ConfigFiles for OrphanHost {
    fn read(&self, file: &ConfigPath) -> Result<Vec<u8>, ConfigFileError> {
        self.1
            .files
            .lock()
            .unwrap()
            .get(file.as_str())
            .cloned()
            .ok_or(ConfigFileError::NotFound)
    }

    fn write(&self, file: &ConfigPath, bytes: &[u8]) -> Result<(), ConfigFileError> {
        self.1
            .files
            .lock()
            .unwrap()
            .insert(file.as_str().into(), bytes.to_vec());
        Ok(())
    }

    fn list(
        &self,
        _: &ConfigPath,
        _: Option<&str>,
        _: u32,
    ) -> Result<Vec<ConfigEntry>, ConfigFileError> {
        panic!("the fixture opens only the exact network data file")
    }

    fn stat(&self, _: &ConfigPath) -> Result<Option<ConfigStat>, ConfigFileError> {
        panic!("the fixture reads the network data file directly")
    }
}

fn fixture_host(material: &'static str) -> OrphanHost {
    mod_api::__private::initialize().unwrap();
    let carrier = Arc::new(Mutex::new(None));
    mod_api::__private::part_tests::install_parts_fixture(&[ModEntry::new(
        "orphan_fixture",
        Box::new(CarrierFixture(material, carrier.clone())),
    )])
    .unwrap();
    OrphanHost(
        FakeHost::new(),
        FixtureWorld {
            carrier: carrier.lock().unwrap().unwrap(),
            material,
            files: Mutex::new(BTreeMap::new()),
        },
    )
}

#[test]
fn an_orphan_gold_pan_without_a_preset_keeps_charge_and_cannot_refill_a_replacement() {
    let mut host = fixture_host(CARRIER);
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let level = host.0.level.level();
    let pos = POSITION;
    let archive = tests::orphan_gold_pan();
    let id = orphan_preset(&archive).unwrap().unwrap();
    let mut legacy = archive.clone();
    legacy.remove("inventory_preset");
    let archived_yaml = archive["paper_inventory"].as_str().unwrap();
    let (_, archived_items) = archived_yaml.split_once('\n').unwrap();
    legacy.insert("paper_inventory".into(), archived_items.into());
    let material = Identifier::parse("minecraft:gravel").unwrap();
    host.0.registries.resolve_items([material.clone()]).unwrap();
    let stack = ItemStack::new(host.item_key(&material).unwrap(), 1).unwrap();
    let networks = Mutex::new(Networks::default());
    let metadata = "energy-charge = '37'".parse::<toml::Table>().unwrap();
    networks
        .lock()
        .unwrap()
        .bind_fixture_node(&mut host, level, pos, id, &metadata, false);
    assert!(restore_orphan_inventory(&mut host, level, pos, &archive, &networks).is_err());
    assert!(restore_orphan_inventory(&mut host, level, pos, &legacy, &networks).is_err());
    assert!(host.0.attachments.to_nbt().is_empty());

    networks
        .lock()
        .unwrap()
        .bind_fixture_node(&mut host, level, pos, id, &metadata, true);
    let before = host.0.attachments.to_nbt();
    let mut conflicting = archive.clone();
    conflicting.insert("inventory_preset".into(), "ELECTRIC_GOLD_PAN".into());
    assert!(restore_orphan_inventory(&mut host, level, pos, &conflicting, &networks).is_err());
    let mut invalid = archive.clone();
    invalid
        .get_mut("paper_items")
        .unwrap()
        .as_array_mut()
        .unwrap()[0]
        .as_table_mut()
        .unwrap()
        .insert("slot".into(), 22.into());
    assert!(restore_orphan_inventory(&mut host, level, pos, &invalid, &networks).is_err());
    let failed = (&mut host as &mut dyn ServerHost).transaction(|host, _| {
        restore_orphan_inventory(host, level, pos, &legacy, &networks).unwrap();
        Err::<(), _>("abort after inventory and receipt")
    });
    assert!(failed.is_err());
    assert_eq!(host.0.attachments.to_nbt(), before);

    restore_orphan_inventory(&mut host, level, pos, &legacy, &networks).unwrap();
    assert_eq!(
        crate::network_runtime::charge(&mut host, level, pos),
        Some(37)
    );
    let mut saved = host.0.attachments.to_nbt();
    let Some(Tag::Compound(slimefun)) = saved.get_mut(&"slimefun".into()) else {
        panic!("Slimefun attachments")
    };
    let Some(Tag::Compound(inventories)) = slimefun.get(&"machine_positions".into()) else {
        panic!("live factory inventory")
    };
    let Some(Tag::Compound(positions)) = inventories.get(&"value".into()) else {
        panic!("factory position index")
    };
    let Some(Tag::Compound(menu)) = positions.get(&"770,112,132".into()) else {
        panic!("restored Gold Pan")
    };
    assert_eq!(
        menu.get(&"19".into()),
        stack.to_nbt(&host).map(Tag::Compound).as_ref()
    );

    // Keep the chunk receipt but replace the original carrier and its inventory.
    slimefun.remove(&"machine_positions".into());
    slimefun.remove(&"plain_energy_nodes".into());
    host.0.attachments = mod_api::Attachments::from_nbt(saved);
    assert!(restore_orphan_inventory(&mut host, level, pos, &archive, &networks).is_err());
    let replacement = "energy-charge = '68'".parse().unwrap();
    networks
        .lock()
        .unwrap()
        .bind_fixture_node(&mut host, level, pos, id, &replacement, true);
    let saved_replacement = host.0.attachments.to_nbt();
    let answer = restore_orphan_inventory(&mut host, level, pos, &archive, &networks).unwrap();
    assert!(answer.contains("already restored"));
    assert_eq!(host.0.attachments.to_nbt(), saved_replacement);
    assert_eq!(
        crate::network_runtime::charge(&mut host, level, pos),
        Some(68)
    );
}

#[test]
fn a_paired_factory_archive_cannot_refill_consumed_or_replaced_inventory() {
    let mut host = fixture_host(CARRIER);
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let level = host.0.level.level();
    let pos = POSITION;
    let mut archive = tests::orphan_gold_pan();
    let id = orphan_preset(&archive).unwrap().unwrap().to_owned();
    let material = Identifier::parse("minecraft:gravel").unwrap();
    host.0.registries.resolve_items([material]).unwrap();
    let metadata = format!("id = '{id}'\nenergy-charge = '37'")
        .parse::<toml::Table>()
        .unwrap();
    archive.insert("metadata".into(), toml::Value::Table(metadata.clone()));
    let networks = Mutex::new(Networks::default());
    networks
        .lock()
        .unwrap()
        .bind_fixture_node(&mut host, level, pos, &id, &metadata, true);
    let original = host.0.attachments.to_nbt();
    let mut invalid = archive.clone();
    invalid
        .get_mut("paper_items")
        .unwrap()
        .as_array_mut()
        .unwrap()[0]
        .as_table_mut()
        .unwrap()
        .insert("slot".into(), 22.into());
    // Both the single and batch commands reach restore_network for paired menus.
    assert!(restore_network(&mut host, level, pos, &invalid, &networks).is_err());
    assert_eq!(host.0.attachments.to_nbt(), original);
    let rollback = (&mut host as &mut dyn ServerHost).transaction(|host, _| {
        restore_network(host, level, pos, &archive, &networks).unwrap();
        Err::<(), _>("abort after saving the menu and chunk receipt")
    });
    assert!(rollback.is_err());
    assert_eq!(host.0.attachments.to_nbt(), original);
    restore_network(&mut host, level, pos, &archive, &networks).unwrap();

    // Simulate withdrawing the restored gravel and pre-receipt persisted data.
    let mut saved = host.0.attachments.to_nbt();
    let Some(Tag::Compound(slimefun)) = saved.get_mut(&"slimefun".into()) else {
        panic!("Slimefun attachments")
    };
    assert!(slimefun.remove(&MENU_RECEIPTS.name.into()).is_some());
    let Some(Tag::Compound(inventories)) = slimefun.get_mut(&"machine_positions".into()) else {
        panic!("live factory inventory")
    };
    let Some(Tag::Compound(positions)) = inventories.get_mut(&"value".into()) else {
        panic!("factory position index")
    };
    let Some(Tag::Compound(menu)) = positions.get_mut(&menu_receipt_key(pos).as_str().into())
    else {
        panic!("restored Gold Pan")
    };
    assert!(menu.remove(&"19".into()).is_some());
    assert!(menu.contains_key(&"paper_inventory_import".into()));
    let consumed = menu.clone();
    host.0.attachments = mod_api::Attachments::from_nbt(saved);
    restore_network(&mut host, level, pos, &archive, &networks).unwrap();

    let mut saved = host.0.attachments.to_nbt();
    let Some(Tag::Compound(slimefun)) = saved.get_mut(&"slimefun".into()) else {
        panic!("Slimefun attachments")
    };
    assert!(slimefun.contains_key(&MENU_RECEIPTS.name.into()));
    assert_eq!(
        slimefun
            .get(&"machine_positions".into())
            .and_then(|entry| entry.get("value"))
            .and_then(|positions| positions.get(menu_receipt_key(pos).as_str())),
        Some(&Tag::Compound(consumed)),
    );

    // Break and replace the Gold Pan. Keep only the restored location receipt.
    slimefun.remove(&"machine_positions".into());
    slimefun.remove(&"plain_energy_nodes".into());
    host.0.attachments = mod_api::Attachments::from_nbt(saved);
    let replacement = "energy-charge = '68'".parse().unwrap();
    networks
        .lock()
        .unwrap()
        .bind_fixture_node(&mut host, level, pos, &id, &replacement, true);
    let before_repeat = host.0.attachments.to_nbt();
    let repeat = restore_network(&mut host, level, pos, &archive, &networks).unwrap();
    assert!(repeat.contains("already restored"), "{repeat}");
    assert!(restore_network(&mut host, level, pos, &invalid, &networks).is_err());
    assert_eq!(host.0.attachments.to_nbt(), before_repeat);
    assert_eq!(
        crate::network_runtime::charge(&mut host, level, pos),
        Some(68)
    );
}

fn saved_cargo(snapshot: &Compound) -> &Tag {
    snapshot
        .get(&"slimefun".into())
        .unwrap()
        .get("network_state")
        .unwrap()
        .get("value")
        .unwrap()
        .get("cargo")
        .unwrap()
}

#[test]
fn paired_cargo_archives_cannot_fill_replacement_filters_after_reload() {
    let mut host = fixture_host("minecraft:player_wall_head");
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let level = host.0.level.level();
    let pos = POSITION;
    let material = Identifier::parse("minecraft:gravel").unwrap();
    host.0.registries.resolve_items([material.clone()]).unwrap();
    let gravel = ItemStack::new(host.item_key(&material).unwrap(), 1).unwrap();
    let owner = "24176106-64f3-4bf2-a9a8-5843823f162a";
    let replacement_owner = "070e909a-2bec-4fcc-b1d8-86c36a484b73";
    for id in ["CARGO_NODE_INPUT", "CARGO_NODE_OUTPUT_ADVANCED"] {
        host.0.attachments = mod_api::Attachments::default();
        host.1.files.lock().unwrap().clear();
        host.0.level.block_entity(pos);
        let mut archive = tests::orphan_gold_pan();
        let original_yaml = archive["paper_inventory"].as_str().unwrap();
        archive.insert(
            "paper_inventory".into(),
            original_yaml.replace("ELECTRIC_GOLD_PAN_2", id).into(),
        );
        archive.insert("inventory_preset".into(), id.into());
        let metadata =
            format!("id = '{id}'\nfrequency = '4'\nfilter-type = 'whitelist'\nowner = '{owner}'");
        archive.insert(
            "metadata".into(),
            toml::Value::Table(metadata.parse().unwrap()),
        );
        let mut invalid = archive.clone();
        invalid
            .get_mut("paper_items")
            .unwrap()
            .as_array_mut()
            .unwrap()[0]
            .as_table_mut()
            .unwrap()
            .insert("slot".into(), 22.into());
        let networks = Mutex::new(Networks::default());
        assert!(restore_network(&mut host, level, pos, &invalid, &networks).is_err());
        assert!(host.0.attachments.to_nbt().is_empty());
        assert!(host.1.files.lock().unwrap().is_empty());
        restore_network(&mut host, level, pos, &archive, &networks).unwrap();
        let mut snapshot = host.0.attachments.to_nbt();
        assert_eq!(
            saved_cargo(&snapshot)
                .get("filters")
                .and_then(|filters| filters.get("19")),
            gravel.to_nbt(&host).map(Tag::Compound).as_ref(),
        );
        assert_eq!(saved_cargo(&snapshot).get("channel"), Some(&Tag::Int(4)));

        // Simulate an already imported old node. The player cleared its filters,
        // but older builds saved only the machine-local paper marker.
        let Tag::Compound(attachments) = snapshot.get_mut(&"slimefun".into()).unwrap() else {
            panic!("Slimefun attachments")
        };
        attachments.remove(&MENU_RECEIPTS.name.into());
        let Tag::Compound(network) = attachments.get_mut(&"network_state".into()).unwrap() else {
            panic!("Cargo node envelope")
        };
        let Tag::Compound(value) = network.get_mut(&"value".into()).unwrap() else {
            panic!("Cargo node state")
        };
        let Tag::Compound(cargo) = value.get_mut(&"cargo".into()).unwrap() else {
            panic!("Cargo settings")
        };
        assert_eq!(cargo.get(&"paper".into()), Some(&Tag::Byte(1)));
        cargo.insert("filters".into(), Tag::Compound(Compound::new()));
        host.0.attachments = mod_api::Attachments::from_nbt(snapshot);
        restore_network(&mut host, level, pos, &archive, &networks).unwrap();
        let mut saved = host.0.attachments.to_nbt();
        assert_eq!(
            saved_cargo(&saved).get("filters"),
            Some(&Tag::Compound(Compound::new())),
        );

        // Breaking the head removes its own attachments, leaving the chunk's
        // receipt. A new node at the same position has its own routing state.
        let Tag::Compound(attachments) = saved.get_mut(&"slimefun".into()).unwrap() else {
            panic!("Slimefun attachments")
        };
        attachments.remove(&"network_state".into());
        host.0.attachments = mod_api::Attachments::from_nbt(saved);
        host.0.level.remove_block_entity(pos);
        host.0.level.block_entity(pos);
        let replacement = format!(
            "[metadata]\nid = '{id}'\nfrequency = '7'\nfilter-type = 'whitelist'\nowner = '{replacement_owner}'"
        ).parse().unwrap();
        restore_network(&mut host, level, pos, &replacement, &networks).unwrap();
        let before = host.0.attachments.to_nbt();
        let persisted_files = host.1.files.lock().unwrap().clone();
        assert_eq!(saved_cargo(&before).get("channel"), Some(&Tag::Int(7)));
        assert_eq!(
            saved_cargo(&before).get("owner"),
            Some(&Tag::String(
                replacement_owner.replace('-', "").as_str().into()
            )),
        );
        host.0.attachments = mod_api::Attachments::from_nbt(before.clone());
        let after_restart = Mutex::new(Networks::default());
        let answer = restore_network(&mut host, level, pos, &archive, &after_restart).unwrap();
        let after = host.0.attachments.to_nbt();
        assert_eq!(
            saved_cargo(&after),
            saved_cargo(&before),
            "{id} changed on replay"
        );
        assert!(answer.contains("already restored"), "{answer}");
        let mut different = archive.clone();
        different
            .get_mut("paper_items")
            .unwrap()
            .as_array_mut()
            .unwrap()[0]
            .as_table_mut()
            .unwrap()
            .insert("slot".into(), 20.into());
        let error = restore_network(&mut host, level, pos, &different, &after_restart).unwrap_err();
        assert!(error.contains("different Paper inventory"), "{error}");
        assert_eq!(after, before);
        assert_eq!(host.0.attachments.to_nbt(), before);
        assert_eq!(*host.1.files.lock().unwrap(), persisted_files);
    }
}
