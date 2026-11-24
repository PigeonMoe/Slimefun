// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, fmt::Write, sync::Mutex};

use mod_api::{
    AttachmentCodec, AttachmentType, CommandArgumentValue, CommandSource, Compound, ConfigPath,
    Holder, HostTransaction, ItemStack, LevelRef, RunModCommandEvent, ServerHost, Tag,
};
use sha2::{Digest, Sha256};

use crate::{
    ancient_altar, block_placer, food_machines,
    geo_deposits::{self, Resource},
    network_runtime::Networks,
    networks::Component,
    output_chest, universal_storage,
};

// One receipt per coordinate, in the same chunk as the restored inventory. The
// receipt survives breaking and replacing the carrier with another copy of the item.
static MENU_RECEIPTS: AttachmentType<Compound> = AttachmentType {
    namespace: "slimefun",
    // Keep the persisted name and string receipts from inventory-only imports.
    name: "paper_orphan_menu_receipts",
    default: Compound::new,
    codec: Some(AttachmentCodec {
        encode: |value| Ok(value.clone()),
        decode: |value| Ok(value.clone()),
    }),
    copy_on_death: false,
    sync: false,
};

fn read(host: &dyn ServerHost, path: &str) -> Result<toml::Table, String> {
    let path = ConfigPath::new(path).map_err(|_| "Invalid Slimefun import file path.")?;
    let data = host
        .world_data_files()
        .read(&path)
        .map_err(|_| "Slimefun could not read the Paper block import file.")?;
    let text = std::str::from_utf8(&data).map_err(|_| "The Paper import file must be UTF-8.")?;
    let table = text
        .parse::<toml::Table>()
        .map_err(|_| "The Paper block import file contains invalid TOML.")?;
    if table.get("format").and_then(toml::Value::as_integer) != Some(1) {
        return Err("Unsupported Slimefun Paper block import version.".into());
    }
    Ok(table)
}

pub(crate) fn encoded_dimension(dimension: &str) -> String {
    let mut encoded = String::with_capacity(dimension.len() * 2);
    for byte in dimension.bytes() {
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

fn chunk_file(dimension: &str, [x, _, z]: [i32; 3]) -> String {
    format!(
        "storage-import/{}/{}_{}.toml",
        encoded_dimension(dimension),
        x.div_euclid(16),
        z.div_euclid(16)
    )
}

fn column([x, _, z]: [i32; 3]) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

pub(crate) fn coordinates(row: &toml::Table) -> Option<[i32; 3]> {
    let [x, y, z] = row.get("position")?.as_array()?.as_slice() else {
        return None;
    };
    let number = |value: &toml::Value| i32::try_from(value.as_integer()?).ok();
    Some([number(x)?, number(y)?, number(z)?])
}

pub(crate) fn decode_inventory(
    host: &dyn ServerHost,
    row: &toml::Table,
) -> Result<Option<BTreeMap<u32, ItemStack>>, String> {
    if !row.contains_key("paper_inventory") {
        return Ok(None);
    }
    decode_archived_slots(host, row, 54).map(Some)
}

pub(crate) fn decode_archived_slots(
    host: &dyn ServerHost,
    row: &toml::Table,
    size: u32,
) -> Result<BTreeMap<u32, ItemStack>, String> {
    if let Some(issues) = row.get("paper_item_issues") {
        let issues = issues.as_array().ok_or("Invalid Paper item diagnostics.")?;
        if let Some(first) = issues.first() {
            let message = first.as_str().ok_or("Invalid Paper item diagnostic.")?;
            return Err(format!("The Paper inventory cannot be restored. {message}"));
        }
    }
    let items = row
        .get("paper_items")
        .and_then(toml::Value::as_array)
        .ok_or("Reimport the Paper archive to convert the inventory slots.")?;
    if items.len() > size as usize
        || row.get("inventory_slots").and_then(toml::Value::as_integer) != Some(items.len() as i64)
    {
        return Err("The Paper inventory has an invalid item count.".into());
    }
    let mut stacks = BTreeMap::new();
    for value in items {
        let entry = value.as_table().ok_or("Invalid Paper item entry.")?;
        let integer = |key| entry.get(key).and_then(toml::Value::as_integer);
        let slot = integer("slot")
            .and_then(|slot| u32::try_from(slot).ok())
            .filter(|slot| *slot < size)
            .ok_or("Invalid Paper item slot.")?;
        let error = || format!("Could not decode the Paper item in slot {slot}.");
        let id = entry
            .get("id")
            .and_then(toml::Value::as_str)
            .ok_or_else(error)?;
        let version = integer("data_version")
            .and_then(|version| i32::try_from(version).ok())
            .ok_or_else(error)?;
        let count = integer("count")
            .and_then(|count| u32::try_from(count).ok())
            .ok_or_else(error)?;
        let components = entry
            .get("components")
            .and_then(toml::Value::as_table)
            .ok_or_else(error)?
            .iter()
            .map(|(id, value)| Some((id.clone(), value.as_str()?.to_owned())))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(error)?;
        let stack = host
            .item_from_saved_components(version, id, count, components)
            .ok_or_else(error)?;
        if stacks.insert(slot, stack).is_some() {
            return Err(format!("The Paper inventory repeats slot {slot}."));
        }
    }
    Ok(stacks)
}

pub(crate) fn validate_inventory_preset(row: &toml::Table, id: &str) -> Result<(), String> {
    match row.get("inventory_preset") {
        Some(toml::Value::String(preset)) if preset == id => Ok(()),
        Some(toml::Value::String(_)) => {
            Err("The Paper menu preset disagrees with the saved block ID.".into())
        }
        Some(_) => Err("The Paper menu preset is invalid.".into()),
        None => Err("The archive does not identify the Paper menu preset.".into()),
    }
}

pub(crate) fn validate_block_inventory_preset(row: &toml::Table, id: &str) -> Result<(), String> {
    // Paper BlockStorage falls back to the block ID when the file has no preset.
    if !row.contains_key("inventory_preset")
        && record_id(row) == Some(id)
        && row.get("paper_inventory").is_some_and(toml::Value::is_str)
        && has_restorable_menu(Component::for_item(id))
        && !crate::auto_crafter::is_machine(id)
    {
        return Ok(());
    }
    validate_inventory_preset(row, id)
}

fn records<'a>(
    chunk: &'a toml::Table,
    dimension: &str,
    expected_column: [i32; 2],
) -> Result<BTreeMap<[i32; 3], &'a toml::Table>, &'static str> {
    if chunk.get("dimension").and_then(toml::Value::as_str) != Some(dimension) {
        return Err("The Paper block import belongs to another dimension.");
    }
    let records = chunk
        .get("records")
        .and_then(toml::Value::as_array)
        .ok_or("The Paper block import has no record list.")?;
    let mut found = BTreeMap::new();
    for value in records {
        let Some(row) = value.as_table() else {
            return Err("The Paper block import has an invalid record.");
        };
        let address = coordinates(row).ok_or("The Paper block import has an invalid position.")?;
        if column(address) != expected_column {
            return Err("The Paper archive contains a block outside its chunk.");
        }
        if found.insert(address, row).is_some() {
            return Err("The Paper block import repeats a block position.");
        }
    }
    Ok(found)
}

fn dimension_and_position(event: &RunModCommandEvent<'_>) -> Option<(LevelRef, [i32; 3])> {
    let CommandArgumentValue::Dimension(level) = event.command.argument("dimension")? else {
        return None;
    };
    let CommandArgumentValue::BlockPos(pos) = event.command.argument("pos")? else {
        return None;
    };
    Some((*level, *pos))
}

pub(crate) fn command(
    host: &mut dyn ServerHost,
    event: &RunModCommandEvent<'_>,
    action: Option<&str>,
    networks: &Mutex<Networks>,
    universal: &universal_storage::UniversalStorage,
) -> Vec<String> {
    if event.source.permission().level() < 2 {
        return vec!["Slimefun storage commands require operator level 2.".into()];
    }
    let result = match action {
        Some(action @ ("restore-universal" | "open-universal")) => {
            let Some(CommandArgumentValue::String(preset)) = event.command.argument("preset")
            else {
                return vec!["Specify the exact Paper universal inventory preset.".into()];
            };
            if action == "restore-universal" {
                universal_storage::restore(host, preset).map(|installed| {
                    vec![if installed {
                        "Restored the Paper universal inventory.".into()
                    } else {
                        "This universal inventory was already restored.".into()
                    }]
                })
            } else if let CommandSource::Player { player, .. } = *event.source {
                universal.open(host, player, preset).map(|()| Vec::new())
            } else {
                Err("Open the universal inventory as an online player.".into())
            }
        }
        None => read(host, "storage-import/index.toml").and_then(|index| {
            let count = |field| {
                index
                    .get(field)
                    .and_then(toml::Value::as_integer)
                    .filter(|count| *count >= 0)
                    .ok_or("Invalid Slimefun storage import index.")
            };
            let mut lines = vec![format!(
                "Paper block import | {} block records | {} inventories | {} chunks",
                count("blocks")?,
                count("inventories")?,
                count("chunks")?
            )];
            if index.contains_key("universal_inventories") || index.contains_key("backpacks") {
                let optional_count = |field| match index.contains_key(field) {
                    true => count(field),
                    false => Ok(0),
                };
                lines.push(format!(
                    "Additional Paper archives | {} universal inventories | {} backpacks",
                    optional_count("universal_inventories")?,
                    optional_count("backpacks")?,
                ));
            }
            if index.contains_key("geo_chunks") {
                lines.push(format!(
                    "Paper GEO | {} scanned chunks",
                    count("geo_chunks")?
                ));
            } else if index.contains_key("oil_reserves") {
                lines.push(format!(
                    "Paper GEO | {} scanned oil reserves",
                    count("oil_reserves")?
                ));
            }
            Ok(lines)
        }),
        Some(action @ ("inspect" | "restore-output" | "restore-network" | "restore-inventory")) => {
            let Some((level, pos)) = dimension_and_position(event) else {
                return vec!["Specify a loaded dimension and block position.".into()];
            };
            at(host, level, pos, action, networks)
        }
        Some(action @ ("restore-geo" | "restore-oil")) => {
            let Some((level, pos)) = dimension_and_position(event) else {
                return vec!["Specify a loaded dimension and block position.".into()];
            };
            restore_geo(host, level, pos, action == "restore-oil")
        }
        Some("restore-chunk") => {
            let Some((level, pos)) = dimension_and_position(event) else {
                return vec!["Specify a loaded dimension and block position.".into()];
            };
            let offset = match event.command.argument("offset") {
                None => 0,
                Some(CommandArgumentValue::Integer(offset)) if *offset >= 0 => *offset as usize,
                _ => return vec!["The restore offset must be a nonnegative integer.".into()],
            };
            restore_chunk(host, level, pos, offset, networks)
        }
        Some(_) => return vec!["Unknown Slimefun storage action.".into()],
    };
    result.unwrap_or_else(|message| vec![message])
}

fn restore_geo(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    oil_only: bool,
) -> Result<Vec<String>, String> {
    let dimension = level
        .dimension()
        .id(host)
        .ok_or("Unknown Horizon dimension.")?;
    let [x, z] = geo_deposits::chunk(pos);
    let file = format!(
        "storage-import/geo/{}/{}_{}.toml",
        encoded_dimension(&dimension),
        x,
        z
    );
    let row = read(host, &file)?;
    let coords = row
        .get("chunk")
        .and_then(toml::Value::as_array)
        .ok_or("Missing Paper GEO chunk coordinates.")?;
    if row.get("dimension").and_then(toml::Value::as_str) != Some(dimension.as_str())
        || coords.len() != 2
        || coords[0].as_integer() != Some(i64::from(x))
        || coords[1].as_integer() != Some(i64::from(z))
    {
        return Err("The Paper GEO archive belongs to another chunk.".into());
    }
    let resources: &[Resource] = if oil_only {
        &[Resource::Oil]
    } else {
        &geo_deposits::SCANNER_RESOURCES
    };
    let mut imported = Vec::with_capacity(resources.len());
    for &resource in resources {
        let Some(amount) = row.get(resource.paper_key()) else {
            continue;
        };
        let amount = amount
            .as_integer()
            .and_then(|amount| i32::try_from(amount).ok())
            .and_then(|amount| u32::try_from(amount).ok())
            .ok_or_else(|| format!("Invalid Paper GEO count for {}.", resource.name()))?;
        imported.push((resource, amount));
    }
    if imported.is_empty() {
        return Err("The Paper archive contains no matching GEO reserves.".into());
    }
    let remaining = geo_deposits::restore(host, level, pos, &imported)?;
    Ok(remaining
        .into_iter()
        .map(|(resource, remaining)| {
            format!(
                "Paper GEO reserve at chunk {x}, {z} | {}: {remaining}",
                resource.name()
            )
        })
        .collect())
}

fn at(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    action: &str,
    networks: &Mutex<Networks>,
) -> Result<Vec<String>, String> {
    let dimension_id = level
        .dimension()
        .id(host)
        .ok_or("Unknown Horizon dimension.")?;
    let dimension = dimension_id.as_str();
    let data = read(host, &chunk_file(dimension, pos))?;
    let records = records(&data, dimension, column(pos))?;
    let row = records
        .get(&pos)
        .copied()
        .ok_or("No Paper Slimefun block record exists at that position.")?;
    let id = record_id(row).unwrap_or("(inventory only)");
    if action == "restore-output" {
        if id != "OUTPUT_CHEST" {
            return Err("That imported record does not identify a Slimefun Output Chest.".into());
        }
        return restore_output(host, level, pos).map(|output| vec![output]);
    }
    if matches!(action, "restore-network" | "restore-inventory") {
        if action == "restore-inventory" && is_orphan_inventory(row) {
            return restore_orphan_inventory(host, level, pos, row, networks)
                .map(|message| vec![message]);
        }
        let id = record_id(row).ok_or("The imported network block needs Paper metadata.")?;
        let component = Component::for_item(id);
        if action == "restore-inventory" && !row.contains_key("paper_inventory") {
            return Err("There is no archived Paper inventory at that position.".into());
        }
        if action == "restore-network" && id == crate::spawner_repair::REINFORCED_SPAWNER.id() {
            return crate::spawner_repair::restore_paper(host, level, pos)
                .map(|restored| vec![restored]);
        }
        if let Some(kind) = ancient_altar::paper_device(id) {
            return ancient_altar::restore_paper(host, level, pos, kind, row)
                .map(|restored| vec![restored]);
        }
        if id == block_placer::BLOCK_PLACER.id() {
            return block_placer::restore_paper(host, level, pos, row)
                .map(|restored| vec![restored]);
        }
        if id == crate::network_recipes::HOLOGRAM_PROJECTOR.id() {
            return crate::hologram_projector::restore_paper(host, level, pos, row)
                .map(|restored| vec![restored]);
        }
        if food_machines::is_placed_product(id) {
            return food_machines::restore_paper(host, level, pos, id, row)
                .map(|restored| vec![restored]);
        }
        if action == "restore-network" && crate::rainbow::block(id).is_some() {
            return crate::rainbow::placed::restore_paper(host, level, pos, id)
                .map(|restored| vec![restored]);
        }
        if action == "restore-inventory" && !has_restorable_menu(component) {
            return Err(format!(
                "Restoring the Paper menu for {id} is not supported yet."
            ));
        }
        return restore_network(host, level, pos, row, networks).map(|restored| vec![restored]);
    }
    let mut lines = vec![format!(
        "Paper Slimefun {} at {dimension} {} {} {}",
        id.escape_debug(),
        pos[0],
        pos[1],
        pos[2]
    )];
    if let Some(slots) = row.get("inventory_slots").and_then(toml::Value::as_integer) {
        lines.push(format!("Archived Paper inventory | {slots} occupied slots"));
        if let Some(preset) = row.get("inventory_preset").and_then(toml::Value::as_str) {
            lines.push(format!("Paper menu preset {}", preset.escape_debug()));
        }
        if let Some(issues) = row.get("paper_item_issues").and_then(toml::Value::as_array) {
            lines.extend(
                issues
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(|issue| format!("Paper inventory migration | {}", issue.escape_debug())),
            );
        }
    }
    if let Some(component) = Component::for_item(id)
        && let Some(metadata) = row.get("metadata").and_then(toml::Value::as_table)
    {
        lines.push(component.describe(metadata));
        if let Some(charge) = crate::network_runtime::charge(host, level, pos) {
            lines.push(match component {
                Component::Capacitor(capacity) => {
                    format!("Active capacitor | {charge} / {capacity} J")
                }
                Component::FuelGenerator { capacity, .. } => {
                    format!("Active generator | {charge} / {capacity} J")
                }
                _ => "Network node active".into(),
            });
        }
    }
    Ok(lines)
}

fn record_id(row: &toml::Table) -> Option<&str> {
    row.get("metadata")?.as_table()?.get("id")?.as_str()
}

pub(crate) fn has_restorable_menu(component: Option<Component>) -> bool {
    matches!(
        component,
        Some(
            Component::ElectricMachine { .. }
                | Component::FuelGenerator { .. }
                | Component::Android
                | Component::ReactorPort
                | Component::CargoInput
                | Component::CargoOutput { .. }
                | Component::TrashCan
        )
    )
}

fn is_orphan_inventory(row: &toml::Table) -> bool {
    !row.contains_key("metadata") && row.contains_key("paper_inventory")
}

fn menu_receipt_key([x, y, z]: [i32; 3]) -> String {
    format!("{x},{y},{z}")
}

fn paired_menu_signature(row: &toml::Table, id: &str) -> Result<Tag, String> {
    // Hash the validated archived slots, independently of world state or a
    // later DataFixer version. The exporter already normalizes component keys.
    let mut items: Vec<_> = row
        .get("paper_items")
        .and_then(toml::Value::as_array)
        .ok_or("The Paper archive has no converted inventory slots.")?
        .iter()
        .collect();
    items.sort_unstable_by_key(|item| item.get("slot").and_then(toml::Value::as_integer));
    let encoded =
        serde_json::to_vec(&items).map_err(|_| "Could not fingerprint the Paper menu.")?;
    let digest = format!("{:x}", Sha256::digest(encoded));
    Ok(Tag::compound([
        ("id", Tag::String(id.into())),
        ("slots_sha256", Tag::String(digest.as_str().into())),
    ]))
}

pub(crate) struct PreparedMenuReceipt {
    holder: Holder,
    entries: Compound,
}

impl PreparedMenuReceipt {
    pub(crate) fn save(self, host: &mut dyn ServerHost) -> Result<(), String> {
        MENU_RECEIPTS
            .set(host, self.holder, self.entries)
            .map_err(|_| "Could not save the Paper inventory receipt.".into())
    }
}

/// Prepare the receipt for a validated menu in the active level. An identical
/// archive has already been used, so repeat callers leave the block alone.
pub(crate) fn prepare_paired_menu_receipt(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    row: &toml::Table,
    id: &str,
) -> Result<Option<PreparedMenuReceipt>, String> {
    let holder = Holder::Chunk(column(pos));
    let mut entries = MENU_RECEIPTS
        .get_existing(host, holder)
        .map_err(|_| "Could not read the Paper inventory receipts.")?
        .unwrap_or_default();
    let key = menu_receipt_key(pos);
    let receipt = paired_menu_signature(row, id)?;
    let legacy = Tag::String(id.into());
    if let Some(previous) = entries.get(&key.as_str().into()) {
        return if previous == &receipt || previous == &legacy {
            Ok(None)
        } else {
            Err("This position already has a receipt for a different Paper inventory.".into())
        };
    }
    entries.insert(key.as_str().into(), receipt);
    Ok(Some(PreparedMenuReceipt { holder, entries }))
}

fn orphan_preset(row: &toml::Table) -> Result<Option<&str>, String> {
    if !is_orphan_inventory(row)
        || row
            .get("paper_inventory")
            .and_then(toml::Value::as_str)
            .is_none()
    {
        return Err("The record is not a valid inventory-only Paper archive.".into());
    }
    let id = match row.get("inventory_preset") {
        Some(toml::Value::String(id)) => id,
        Some(_) => return Err("The Paper menu preset is invalid.".into()),
        None => return Ok(None),
    };
    if !has_restorable_menu(Component::for_item(id)) || crate::auto_crafter::is_machine(id) {
        return Err("This inventory-only Paper menu preset is unsupported.".into());
    }
    Ok(Some(id))
}

fn restore_orphan_inventory(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    row: &toml::Table,
    networks: &Mutex<Networks>,
) -> Result<String, String> {
    let preset = orphan_preset(row)?;
    let items = decode_archived_slots(host, row, 54)?;
    let mut networks = networks.lock().unwrap();
    host.transaction(|host, _| {
        // Paper uses the registered block ID to select the menu when an older
        // .sfi has no preset. The block and its saved identity must agree.
        let (id, component) = networks.installed_menu(host, level, pos, preset)?;
        if !has_restorable_menu(Some(component)) || crate::auto_crafter::is_machine(&id) {
            return Err("This inventory-only Paper menu preset is unsupported.".into());
        }
        let holder = Holder::Chunk(column(pos));
        let mut receipts = MENU_RECEIPTS
            .get_existing(host, holder)
            .map_err(|_| "Could not read the Paper inventory receipts.")?
            .unwrap_or_default();
        let key = menu_receipt_key(pos);
        let receipt = Tag::String(id.as_str().into());
        if let Some(previous) = receipts.get(&key.as_str().into()) {
            return if previous == &receipt {
                Ok(format!(
                    "The inventory-only Paper archive for {id} was already restored."
                ))
            } else {
                Err("This position already has a receipt for a different Paper inventory.".into())
            };
        }

        crate::network_runtime::restore_installed_menu(host, level, pos, &id, component, items)?;
        receipts.insert(key.as_str().into(), receipt);
        MENU_RECEIPTS
            .set(host, holder, receipts)
            .map_err(|_| "Could not save the Paper inventory receipt.")?;
        Ok(format!(
            "Restored the Paper inventory for the existing {id} block."
        ))
    })
}

fn restore_output(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> Result<String, String> {
    output_chest::restore_paper_marker(host, level, pos)
        .then(|| "Restored the Slimefun Output Chest marker.".into())
        .ok_or_else(|| {
            "Load the imported world chest in the active dimension before restoring it.".into()
        })
}

fn restore_network(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    row: &toml::Table,
    networks: &Mutex<Networks>,
) -> Result<String, String> {
    let metadata = row
        .get("metadata")
        .and_then(toml::Value::as_table)
        .ok_or("The imported network block needs Paper metadata.")?;
    let id = metadata
        .get("id")
        .and_then(toml::Value::as_str)
        .ok_or("The Paper block has no Slimefun ID.")?;
    let mut networks = networks.lock().unwrap();
    if crate::auto_crafter::is_machine(id) {
        return networks.restore(host, level, pos, id, metadata, row);
    }
    if !matches!(
        Component::for_item(id),
        Some(
            Component::ElectricMachine { .. }
                | Component::FuelGenerator { .. }
                | Component::ReactorPort
        )
    ) {
        return networks.restore(host, level, pos, id, metadata, row);
    }
    let inventory = decode_inventory(host, row)?;
    let pending_receipts = if inventory.is_some() {
        validate_block_inventory_preset(row, id)?;
        if !level.in_place(host) {
            return Err("The electric machine is not supported in this level.".into());
        }
        let Some(receipt) = prepare_paired_menu_receipt(host, pos, row, id)? else {
            return Ok(format!(
                "The Paper inventory for {id} was already restored."
            ));
        };
        Some(receipt)
    } else {
        None
    };
    let prepared = crate::factory::prepare_paper_inventory(
        host,
        level,
        pos,
        id,
        inventory.unwrap_or_default(),
    )?;
    let count = prepared.len();
    host.transaction(|host, _| {
        let has_archive = pending_receipts.is_some();
        if let Some(receipt) = pending_receipts {
            receipt.save(host)?;
        }
        let restored = networks.restore(host, level, pos, id, metadata, row)?;
        crate::factory::restore_paper_assembler_controls(host, level, pos, id, metadata)?;
        if has_archive {
            crate::factory::restore_paper_inventory(host, level, pos, id, prepared)?;
            Ok(format!(
                "Restored {id} from {count} archived Paper menu slots."
            ))
        } else {
            Ok(restored)
        }
    })
}

fn restore_chunk(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    offset: usize,
    networks: &Mutex<Networks>,
) -> Result<Vec<String>, String> {
    const BATCH: usize = 32;
    const BATCH_MILLIS: u64 = 25;
    let started = host.server_monotonic_millis();
    let chunk = column(pos);
    if !level.in_place(host) || host.chunk(level, chunk).is_none() {
        return Err("Load the restore chunk in the active dimension first.".into());
    }
    let dimension = level
        .dimension()
        .id(host)
        .ok_or("Unknown Horizon dimension.")?;
    let data = read(host, &chunk_file(&dimension, pos))?;
    let archived = records(&data, &dimension, chunk)?;
    let archived_count = archived.len();
    let targets: Vec<_> = archived
        .into_iter()
        .filter_map(|(pos, row)| {
            if is_orphan_inventory(row) {
                return Some((pos, row, "(inventory only)"));
            }
            let id = record_id(row)?;
            (id == "OUTPUT_CHEST"
                || id == block_placer::BLOCK_PLACER.id()
                || id == crate::spawner_repair::REINFORCED_SPAWNER.id()
                || id == crate::network_recipes::HOLOGRAM_PROJECTOR.id()
                || Component::for_item(id).is_some()
                || food_machines::is_placed_product(id)
                || crate::rainbow::block(id).is_some()
                || ancient_altar::paper_device(id).is_some())
            .then_some((pos, row, id))
        })
        .collect();
    let total = targets.len();
    if total == 0 {
        return Ok(vec![format!(
            "The chunk contains {archived_count} Paper records and no supported restoration.",
        )]);
    }
    if offset >= total {
        return Err("The restore offset is past the last supported block in this chunk.".into());
    }
    let limit = total.min(offset + BATCH);
    let mut end = offset;
    let mut failures = Vec::new();
    for &(pos, row, id) in &targets[offset..limit] {
        // Finish each record before pausing and return the next untouched offset.
        // The command shares the server's short Wasm hook deadline.
        if end != offset && host.server_monotonic_millis().saturating_sub(started) >= BATCH_MILLIS {
            break;
        }
        let unsupported_menu = !has_restorable_menu(Component::for_item(id))
            && row
                .get("inventory_slots")
                .and_then(toml::Value::as_integer)
                .is_some_and(|count| count != 0);
        let result = if is_orphan_inventory(row) {
            restore_orphan_inventory(host, level, pos, row, networks)
        } else if let Some(kind) = ancient_altar::paper_device(id) {
            ancient_altar::restore_paper(host, level, pos, kind, row)
        } else if id == block_placer::BLOCK_PLACER.id() {
            block_placer::restore_paper(host, level, pos, row)
        } else if id == crate::network_recipes::HOLOGRAM_PROJECTOR.id() {
            crate::hologram_projector::restore_paper(host, level, pos, row)
        } else if food_machines::is_placed_product(id) {
            food_machines::restore_paper(host, level, pos, id, row)
        } else if unsupported_menu {
            Err("This block has archived items without a supported menu.".into())
        } else if id == crate::spawner_repair::REINFORCED_SPAWNER.id() {
            crate::spawner_repair::restore_paper(host, level, pos)
        } else if crate::rainbow::block(id).is_some() {
            crate::rainbow::placed::restore_paper(host, level, pos, id)
        } else if id == "OUTPUT_CHEST" {
            restore_output(host, level, pos)
        } else {
            restore_network(host, level, pos, row, networks)
        };
        if let Err(error) = result {
            let [x, y, z] = pos;
            failures.push(format!("{x} {y} {z} ({id}) | {error}"));
        }
        end += 1;
    }
    let restored = end - offset - failures.len();
    let mut lines = vec![format!(
        "Paper chunk {}, {} | {restored} restored | {} refused | records {}–{end} of {total} supported",
        chunk[0],
        chunk[1],
        failures.len(),
        offset + 1,
    )];
    if archived_count > total {
        lines.push(format!(
            "{} unsupported Paper records remain archived in this chunk.",
            archived_count - total,
        ));
    }
    lines.extend(failures);
    if end < total {
        let [x, y, z] = pos;
        lines.push(format!(
            "/sf storage restore-chunk {dimension} {x} {y} {z} {end}"
        ));
    }
    Ok(lines)
}

#[cfg(test)]
#[path = "storage_orphan_tests.rs"]
mod orphan_live_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_negative_chunks_and_refuses_duplicate_paper_records() {
        assert_eq!(
            chunk_file("minecraft:overworld", [-1, 64, -17]),
            "storage-import/6d696e6563726166743a6f766572776f726c64/-1_-2.toml"
        );
        let header = "dimension='minecraft:overworld'\n";
        let row = "[[records]]\nposition = [-1, 64, -17]\n[records.metadata]\nid='OUTPUT_CHEST'\n";
        let chunk = format!("{header}{row}").parse().unwrap();
        assert!(
            records(&chunk, "minecraft:overworld", [-1, -2])
                .unwrap()
                .contains_key(&[-1, 64, -17])
        );
        let duplicated = format!("{header}{row}{row}").parse().unwrap();
        assert!(records(&duplicated, "minecraft:overworld", [-1, -2]).is_err());
        assert!(records(&chunk, "minecraft:the_end", [-1, -2]).is_err());
        assert!(records(&chunk, "minecraft:overworld", [-1, -1]).is_err());
    }

    #[test]
    fn empty_archived_menus_are_distinct_from_absent_or_unsupported_menus() {
        let host = mod_api::testing::FakeHost::new();
        let row: toml::Table = "paper_inventory = 'preserved archive'\ninventory_preset = 'CARGO_NODE_INPUT'\ninventory_slots = 0\npaper_items = []"
            .parse().unwrap();
        assert!(decode_inventory(&host, &row).unwrap().unwrap().is_empty());
        assert!(validate_inventory_preset(&row, "CARGO_NODE_INPUT").is_ok());
        assert!(validate_inventory_preset(&row, "CARGO_NODE_OUTPUT").is_err());
        assert!(
            decode_inventory(&host, &toml::Table::new())
                .unwrap()
                .is_none()
        );

        let mut old = row.clone();
        old.remove("paper_items");
        assert!(
            decode_inventory(&host, &old)
                .unwrap_err()
                .contains("Reimport")
        );
        old.insert(
            "paper_item_issues".into(),
            vec!["Slot 19: legacy item"].into(),
        );
        assert!(
            decode_inventory(&host, &old)
                .unwrap_err()
                .contains("Slot 19")
        );
        old.remove("paper_item_issues");
        old.insert("paper_items".into(), toml::Value::Array(vec![]));
        old.insert("inventory_slots".into(), 1.into());
        assert!(decode_inventory(&host, &old).is_err());
    }

    #[test]
    fn a_presetless_paper_inventory_can_use_its_matching_stock_block_metadata() {
        let archived = |id: &str| {
            format!(
                "paper_inventory = 'original Paper file'\n\
                 inventory_slots = 1\n\
                 paper_items = [{{slot = 19, id = 'minecraft:stone', count = 1, data_version = 4903, components = {{}}}}]\n\
                 [metadata]\nid = '{id}'"
            )
            .parse::<toml::Table>()
            .unwrap()
        };
        for id in [
            "ELECTRIC_FURNACE",
            "CARGO_NODE_INPUT",
            "CARGO_NODE_OUTPUT_ADVANCED",
            "CARGO_NODE_OUTPUT",
        ] {
            assert!(
                validate_block_inventory_preset(&archived(id), id).is_ok(),
                "{id}"
            );
        }

        let mut row = archived("ELECTRIC_FURNACE");
        assert!(validate_inventory_preset(&row, "ELECTRIC_FURNACE").is_err());
        assert!(validate_block_inventory_preset(&row, "CARGO_NODE_INPUT").is_err());
        row.insert("inventory_preset".into(), "ELECTRIC_FURNACE_2".into());
        assert!(validate_block_inventory_preset(&row, "ELECTRIC_FURNACE").is_err());
        row.insert("inventory_preset".into(), false.into());
        assert!(validate_block_inventory_preset(&row, "ELECTRIC_FURNACE").is_err());
        row.remove("inventory_preset");
        row.remove("metadata");
        assert!(validate_block_inventory_preset(&row, "ELECTRIC_FURNACE").is_err());
        for id in ["EXTENSION_MENU", "CARGO_MANAGER", "ENHANCED_AUTO_CRAFTER"] {
            assert!(
                validate_block_inventory_preset(&archived(id), id).is_err(),
                "{id}"
            );
        }
    }

    pub(super) fn orphan_gold_pan() -> toml::Table {
        let mut archive: toml::Table = r#"
            inventory_preset = "ELECTRIC_GOLD_PAN_2"
            inventory_slots = 1
            paper_items = [{slot = 19, id = "minecraft:gravel", count = 1, data_version = 4903, components = {}}]
        "#
        .parse()
        .unwrap();
        archive.insert(
            "paper_inventory".into(),
            include_str!("../tools/paper-storage-export/fixtures/paper-26.2-electric-gold-pan.sfi")
                .into(),
        );
        archive
    }

    #[test]
    fn orphan_menus_reject_unsupported_explicit_presets_and_metadata() {
        let mut row = orphan_gold_pan();
        assert_eq!(orphan_preset(&row).unwrap(), Some("ELECTRIC_GOLD_PAN_2"));
        for invalid in [
            true.into(),
            "EXTENSION_MENU".into(),
            "ENHANCED_AUTO_CRAFTER".into(),
        ] {
            row.insert("inventory_preset".into(), invalid);
            assert!(orphan_preset(&row).is_err());
        }
        row.remove("inventory_preset");
        assert_eq!(orphan_preset(&row).unwrap(), None);
        let mut row = orphan_gold_pan();
        row.insert("metadata".into(), toml::Value::Table(toml::Table::new()));
        assert!(!is_orphan_inventory(&row));
        assert!(orphan_preset(&row).is_err());
    }

    #[test]
    fn supported_paper_menus_can_be_restored_individually_and_in_batches() {
        for machine in [
            "HEATED_PRESSURE_CHAMBER",
            "HEATED_PRESSURE_CHAMBER_2",
            "ELECTRIC_FURNACE_2",
            "ELECTRIC_ORE_GRINDER_3",
            "AUTO_ENCHANTER",
            "AUTO_ENCHANTER_2",
            "AUTO_DISENCHANTER",
            "AUTO_DISENCHANTER_2",
            "AUTO_ANVIL",
            "AUTO_ANVIL_2",
            "BOOK_BINDER",
            "LAVA_GENERATOR_2",
            "BIO_REACTOR",
            "REACTOR_ACCESS_PORT",
            "AUTO_BREEDER",
            "ANIMAL_GROWTH_ACCELERATOR",
            "PROGRAMMABLE_ANDROID",
            "PROGRAMMABLE_ANDROID_FARMER",
            "CROP_GROWTH_ACCELERATOR",
            "CROP_GROWTH_ACCELERATOR_2",
            "CARGO_NODE_INPUT",
            "CARGO_NODE_OUTPUT_ADVANCED",
            "CARGO_NODE_OUTPUT",
            "TRASH_CAN_BLOCK",
        ] {
            assert!(
                has_restorable_menu(Component::for_item(machine)),
                "{machine}"
            );
        }
        for role in ["OUTPUT_CHEST", "ENERGY_REGULATOR", "CARGO_MANAGER"] {
            assert!(!has_restorable_menu(Component::for_item(role)), "{role}");
        }
    }
}
