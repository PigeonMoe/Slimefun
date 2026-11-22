// SPDX-License-Identifier: GPL-3.0-only

//! The Paper Slimefun section of a PM-Sync snapshot (SFP1).

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, LazyLock, Mutex},
};

use mod_api::{
    Compound, ConfigFileError, ConfigPath, Host, Identifier, ItemStack, ServerHost, Tag,
    TransferProfileReadError,
};
use sha2::{Digest, Sha256};

use crate::{
    backpacks::{self, CaptureBags},
    research,
};

const MAX_PROFILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ITEM_BYTES: usize = 1024 * 1024;
const MAX_RESEARCHES: usize = 16_384;
const MAX_RESEARCH_KEY_BYTES: usize = 512;
const MAX_BACKPACKS: usize = 4_096;
const DIGEST_BYTES: usize = 32;
const OVERSIZE: &str = "Slimefun 同步档案大小超出允许范围。";
const INVALID_DATA: &str = "Slimefun 同步档案包含无效数据。";
const TRUNCATED: &str = "Slimefun 同步档案不完整。";
static SECTION: LazyLock<Identifier> =
    LazyLock::new(|| Identifier::parse("slimefun:profile").expect("Slimefun transfer section"));
static COMMITTED_CACHE: Mutex<Option<CachedProfile>> = Mutex::new(None);

struct CachedProfile {
    owner: u128,
    generation: u128,
    profile: Arc<Profile<'static>>,
}

fn committed_profile_error(error: TransferProfileReadError) -> String {
    match error {
        TransferProfileReadError::Transferring | TransferProfileReadError::Away => {
            "玩家正在换服或已离开本服，暂时无法读取 Slimefun 档案。".into()
        }
        _ => "无法验证本服当前的 Slimefun 档案。".into(),
    }
}

pub(crate) fn committed_generation(
    host: &dyn ServerHost,
    owner: u128,
) -> Result<Option<u128>, String> {
    host.player_transfer_profile_generation(owner, &SECTION)
        .map_err(committed_profile_error)
}

pub(crate) fn with_committed_profile<T>(
    host: &dyn ServerHost,
    owner: u128,
    generation: u128,
    read: impl FnOnce(&Profile<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let profile = committed_profile(host, owner, generation)?;
    read(&profile)
}

fn committed_profile(
    host: &dyn ServerHost,
    owner: u128,
    generation: u128,
) -> Result<Arc<Profile<'static>>, String> {
    // Callers selected the active generation through the host. The engine owns
    // the immutable payload, so many slot reads can share one validated decode.
    let cached = COMMITTED_CACHE
        .lock()
        .unwrap()
        .as_ref()
        .filter(|cached| cached.owner == owner && cached.generation == generation)
        .map(|cached| Arc::clone(&cached.profile));
    if let Some(profile) = cached {
        return Ok(profile);
    }
    let bytes = host
        .player_transfer_profile_payload(owner, &SECTION, generation)
        .map_err(committed_profile_error)?;
    let profile = Arc::new(Profile::decode(owner, &bytes)?.into_owned());
    *COMMITTED_CACHE.lock().unwrap() = Some(CachedProfile {
        owner,
        generation,
        profile: Arc::clone(&profile),
    });
    Ok(profile)
}

pub(crate) fn clear_committed_cache() {
    *COMMITTED_CACHE.lock().unwrap() = None;
}

#[derive(Clone, Default)]
pub(crate) struct Profile<'a> {
    pub(crate) guide_issued: bool,
    pub(crate) researches: BTreeSet<String>,
    pub(crate) legacy_researches: BTreeSet<i32>,
    /// Number → every slot in original order. Empty bytes denote an empty slot.
    pub(crate) backpacks: BTreeMap<u32, Vec<Cow<'a, [u8]>>>,
}

impl<'a> Profile<'a> {
    fn into_owned(self) -> Profile<'static> {
        Profile {
            guide_issued: self.guide_issued,
            researches: self.researches,
            legacy_researches: self.legacy_researches,
            backpacks: self
                .backpacks
                .into_iter()
                .map(|(id, slots)| {
                    (
                        id,
                        slots
                            .into_iter()
                            .map(|item| Cow::Owned(item.into_owned()))
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    /// Match Paper's registered keys and each of the IDs shared by multiple
    /// Research definitions. Unknown add-on values remain in the source file.
    pub(crate) fn slimefun_researches(&self) -> BTreeSet<String> {
        paper_researches()
            .filter_map(|(id, legacy)| {
                let key = format!("slimefun:{id}");
                (self.researches.contains(&key) || self.legacy_researches.contains(&legacy))
                    .then_some(key)
            })
            .collect()
    }

    pub(crate) fn decode(owner: u128, bytes: &'a [u8]) -> Result<Self, &'static str> {
        if !(64..=MAX_PROFILE_BYTES).contains(&bytes.len()) {
            return Err(OVERSIZE);
        }
        let (payload, checksum) = bytes.split_at(bytes.len() - DIGEST_BYTES);
        if Sha256::digest(payload)[..] != *checksum {
            return Err("Slimefun 同步档案的 SHA-256 校验失败。");
        }
        let mut input = Input(payload);
        if input.take(4)? != b"SFP1" {
            return Err("不支持此 Slimefun 同步档案版本。");
        }
        if input.take(16)? != owner.to_be_bytes() {
            return Err("Slimefun 同步档案的玩家 UUID 不匹配。");
        }
        let guide_issued = match input.take(1)? {
            [0] => false,
            [1] => true,
            _ => return Err(INVALID_DATA),
        };
        let mut researches = BTreeSet::new();
        for _ in 0..input.count(MAX_RESEARCHES)? {
            if !researches.insert(input.research_key()?) {
                return Err(INVALID_DATA);
            }
        }
        let mut legacy_researches = BTreeSet::new();
        for _ in 0..input.count(MAX_RESEARCHES)? {
            if !legacy_researches.insert(input.integer()?) {
                return Err(INVALID_DATA);
            }
        }
        let mut backpacks = BTreeMap::new();
        for _ in 0..input.count(MAX_BACKPACKS)? {
            let id = u32::try_from(input.integer()?).map_err(|_| INVALID_DATA)?;
            let size = input.count(54)?;
            if size == 0 || size % 9 != 0 || backpacks.contains_key(&id) {
                return Err(INVALID_DATA);
            }
            let mut slots = Vec::with_capacity(size);
            for _ in 0..size {
                let length = input.count(MAX_ITEM_BYTES)?;
                slots.push(Cow::Borrowed(input.take(length)?));
            }
            backpacks.insert(id, slots);
        }
        if !input.0.is_empty() {
            return Err("Slimefun 同步档案末尾含有额外数据。");
        }
        Ok(Self {
            guide_issued,
            researches,
            legacy_researches,
            backpacks,
        })
    }

    pub(crate) fn encode(&self, owner: u128) -> Result<Vec<u8>, &'static str> {
        let mut output = Output(Vec::new());
        output.write(b"SFP1")?;
        output.write(&owner.to_be_bytes())?;
        output.write(&[u8::from(self.guide_issued)])?;

        output.count(self.researches.len(), MAX_RESEARCHES)?;
        for key in &self.researches {
            if !valid_research_key(key) {
                return Err(INVALID_DATA);
            }
            // All valid resource key characters have the same bytes in ASCII and Java MUTF-8.
            output.write(&(key.len() as u16).to_be_bytes())?;
            output.write(key.as_bytes())?;
        }
        output.count(self.legacy_researches.len(), MAX_RESEARCHES)?;
        for id in &self.legacy_researches {
            output.write(&id.to_be_bytes())?;
        }
        output.count(self.backpacks.len(), MAX_BACKPACKS)?;
        for (&id, slots) in &self.backpacks {
            output.write(&i32::try_from(id).map_err(|_| INVALID_DATA)?.to_be_bytes())?;
            if slots.is_empty() || slots.len() % 9 != 0 {
                return Err(INVALID_DATA);
            }
            output.count(slots.len(), 54)?;
            for item in slots {
                output.count(item.len(), MAX_ITEM_BYTES)?;
                output.write(item)?;
            }
        }
        let digest = Sha256::digest(&output.0);
        output.0.extend_from_slice(&digest);
        Ok(output.0)
    }
}

pub(crate) fn paper_researches() -> impl Iterator<Item = (&'static str, i32)> {
    include_str!("knowledge_researches.tsv")
        .lines()
        .filter_map(|row| {
            let (id, rest) = row.split_once('\t')?;
            let (_, legacy) = rest.rsplit_once('\t')?;
            Some((id, legacy.parse().ok()?))
        })
}

struct Output(Vec<u8>);

impl Output {
    fn write(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        if bytes.len() > MAX_PROFILE_BYTES - DIGEST_BYTES - self.0.len() {
            return Err(OVERSIZE);
        }
        self.0.extend_from_slice(bytes);
        Ok(())
    }

    fn count(&mut self, length: usize, max: usize) -> Result<(), &'static str> {
        if length > max {
            return Err(INVALID_DATA);
        }
        self.write(&(length as i32).to_be_bytes())
    }
}

struct Input<'a>(&'a [u8]);

impl<'a> Input<'a> {
    fn take(&mut self, amount: usize) -> Result<&'a [u8], &'static str> {
        let (value, rest) = self.0.split_at_checked(amount).ok_or(TRUNCATED)?;
        self.0 = rest;
        Ok(value)
    }

    fn integer(&mut self) -> Result<i32, &'static str> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn count(&mut self, limit: usize) -> Result<usize, &'static str> {
        usize::try_from(self.integer()?)
            .ok()
            .filter(|value| *value <= limit)
            .ok_or(INVALID_DATA)
    }

    fn research_key(&mut self) -> Result<String, &'static str> {
        let length = u16::from_be_bytes(self.take(2)?.try_into().unwrap());
        // DataInput.readUTF accepts overlong encodings. Paper's later identifier
        // check still restricts every decoded character to ASCII.
        let mut bytes = Input(self.take(usize::from(length))?);
        let mut key = String::with_capacity(usize::from(length).min(MAX_RESEARCH_KEY_BYTES));
        while !bytes.0.is_empty() {
            let first = bytes.take(1)?[0];
            let unit = match first {
                0..=127 => u16::from(first),
                192..=223 => (u16::from(first & 31) << 6) | bytes.utf_continuation()?,
                224..=239 => {
                    (u16::from(first & 15) << 12)
                        | (bytes.utf_continuation()? << 6)
                        | bytes.utf_continuation()?
                }
                _ => return Err(INVALID_DATA),
            };
            if unit > 127 || key.len() == MAX_RESEARCH_KEY_BYTES {
                return Err(INVALID_DATA);
            }
            key.push(char::from(unit as u8));
        }
        if !valid_research_key(&key) {
            return Err(INVALID_DATA);
        }
        Ok(key)
    }

    fn utf_continuation(&mut self) -> Result<u16, &'static str> {
        let byte = self.take(1)?[0];
        (byte & 0xc0 == 0x80)
            .then_some(u16::from(byte & 63))
            .ok_or(INVALID_DATA)
    }
}

fn valid_research_key(key: &str) -> bool {
    let valid = |part: &str, allow_slash: bool| {
        !part.is_empty()
            && part.bytes().all(|byte| {
                matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-')
                    || allow_slash && byte == b'/'
            })
    };
    key.len() <= MAX_RESEARCH_KEY_BYTES
        && key
            .split_once(':')
            .is_some_and(|(namespace, path)| valid(namespace, false) && valid(path, true))
}

fn read_archive(host: &dyn ServerHost, owner: u128) -> Result<Option<Vec<u8>>, String> {
    let path = ConfigPath::new(&format!("pm-sync-import/{owner:032x}.sfp1"))
        .expect("a hexadecimal UUID is a valid archive path");
    match host.world_data_files().read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(ConfigFileError::NotFound) => Ok(None),
        Err(error) => Err(format!("无法读取 Slimefun 同步档案。{error}")),
    }
}

pub(crate) fn with_archive<T>(
    host: &dyn ServerHost,
    owner: u128,
    read: impl FnOnce(Profile<'_>) -> Result<T, String>,
) -> Result<Option<T>, String> {
    let Some(bytes) = read_archive(host, owner)? else {
        return Ok(None);
    };
    read(Profile::decode(owner, &bytes)?).map(Some)
}

#[derive(Debug)]
pub(crate) struct StagedProfile {
    pub(crate) owner: u128,
    pub(crate) player_data: Compound,
    pub(crate) guide_issued: bool,
    pub(crate) unknown_researches: BTreeSet<String>,
    pub(crate) unknown_legacy_researches: BTreeSet<i32>,
    backpack_records: BTreeMap<u32, Compound>,
}

impl StagedProfile {
    /// Project the owner-wide replacement onto the current map, read under the
    /// coordinator's commit fence. Do not reuse a map read before staging or
    /// allow archival recovery for a missing record of this synchronized owner.
    pub(crate) fn replace_backpack_records(&self, mut records: Compound) -> Compound {
        let prefix = format!("{:032x}#", self.owner);
        records.retain(|key, _| !key.to_string_lossy().starts_with(&prefix));
        records.extend(self.backpack_records.iter().map(|(&number, data)| {
            let id = backpacks::BackpackId {
                owner: self.owner,
                number,
            };
            (
                id.storage_key().as_str().into(),
                Tag::Compound(data.clone()),
            )
        }));
        records
    }
}

/// Validate the complete incoming profile against the held native player's
/// candidate PDC without publishing it. The coordinator must apply the result
/// for its claimed revision before interaction, fence old bag viewers and paid
/// study callbacks, and bypass legacy player/tome/bag imports for the owner.
/// Server-local GPS data stays outside this section and is loaded independently.
pub(crate) fn stage_received_profile(
    host: &dyn ServerHost,
    owner: u128,
    id: &Identifier,
    schema: u32,
    candidate: &Compound,
    payload: &[u8],
) -> Result<Compound, String> {
    if id != &*SECTION || schema != 1 {
        return Err("不支持此 Slimefun 同步档案版本。".into());
    }
    let mut projected = stage_apply(host, owner, candidate, payload)?.player_data;
    match projected.remove(&"PlayerPersisted".into()) {
        Some(Tag::Compound(entries)) => Ok(entries),
        _ => Err("无法准备 Slimefun 玩家同步数据。".into()),
    }
}

pub(crate) fn stage_apply(
    host: &dyn ServerHost,
    owner: u128,
    base_player_data: &Compound,
    bytes: &[u8],
) -> Result<StagedProfile, String> {
    stage_apply_with_decoder(host, owner, base_player_data, bytes, |bytes| {
        host.item_from_saved_gzip_nbt(bytes.to_vec())
    })
}

fn stage_apply_with_decoder(
    host: &dyn Host,
    owner: u128,
    base_player_data: &Compound,
    bytes: &[u8],
    mut decode: impl FnMut(&[u8]) -> Option<ItemStack>,
) -> Result<StagedProfile, String> {
    let mut profile = Profile::decode(owner, bytes)?;
    let researches = profile.slimefun_researches();
    let player_data =
        research::stage_replacement(base_player_data, &researches, profile.guide_issued)?;

    let mut backpack_records = BTreeMap::new();
    for (number, slots) in profile.backpacks {
        let staged = backpacks::stage_snapshot_bag(
            host,
            number,
            slots.iter().map(Cow::as_ref),
            &mut decode,
        )?;
        backpack_records.insert(number, staged);
    }
    // Resolve matches against the full incoming legacy set first, so every
    // registered research sharing a historical number acquires the unlock.
    for (id, legacy) in paper_researches() {
        profile.researches.remove(&format!("slimefun:{id}"));
        profile.legacy_researches.remove(&legacy);
    }
    Ok(StagedProfile {
        owner,
        player_data,
        guide_issued: profile.guide_issued,
        unknown_researches: profile.researches,
        unknown_legacy_researches: profile.legacy_researches,
        backpack_records,
    })
}

/// Take the Slimefun section after the coordinator has settled the first-guide
/// lifecycle and prepared the current online player for transfer. The caller
/// holds the interaction lock and keeps it through capture of other sections.
pub(crate) fn capture_prepared(
    host: &dyn ServerHost,
    owner: u128,
    id: &Identifier,
    schema: u32,
) -> Result<Vec<u8>, String> {
    if id != &*SECTION || schema != 1 {
        return Err("不支持此 Slimefun 同步档案版本。".into());
    }
    let Some(player) = host.player(owner) else {
        return Err("只能采集当前在线会话的 Slimefun 档案。".into());
    };
    let data = player
        .persistent_data(host)
        .ok_or("无法读取当前玩家的研究档案。")?;
    let generation = committed_generation(host, owner)?;
    if let Some(generation) = generation {
        let profile = committed_profile(host, owner, generation)?;
        return capture_state(
            host,
            owner,
            &data,
            (*profile).clone(),
            CaptureBags::Committed(generation),
            |host, bytes| host.item_from_saved_gzip_nbt(bytes.to_vec()).is_some(),
            |host, item| host.item_to_saved_gzip_nbt(item),
        );
    }
    let archive = read_archive(host, owner)?;
    let profile = archive
        .as_deref()
        .map(|bytes| Profile::decode(owner, bytes))
        .transpose()?
        .unwrap_or_default();
    let paper = backpacks::read_profile(host, owner)?;
    capture_state(
        host,
        owner,
        &data,
        profile,
        CaptureBags::Legacy(paper.as_ref()),
        |host, bytes| host.item_from_saved_gzip_nbt(bytes.to_vec()).is_some(),
        |host, item| host.item_to_saved_gzip_nbt(item),
    )
}

fn capture_state(
    host: &dyn ServerHost,
    owner: u128,
    data: &Compound,
    mut profile: Profile<'_>,
    bags: CaptureBags<'_>,
    decode_item: impl Fn(&dyn ServerHost, &[u8]) -> bool,
    encode_item: impl Fn(&dyn ServerHost, &ItemStack) -> Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    research::update_capture(data, &mut profile)?;
    if matches!(bags, CaptureBags::Legacy(Some(_))) {
        // Match lazy restoration: a normalized Paper profile takes precedence
        // over the SFP1 archive for bags, and live records win for both formats.
        profile.backpacks.clear();
    }
    let mut encoded_bytes = 0;
    backpacks::visit_snapshot_bags(host, owner, bags, MAX_BACKPACKS, |host, id, bag| {
        let mut slots = vec![Cow::Borrowed(&[][..]); bag.size as usize];
        for (slot, item) in bag.slots {
            let bytes = encode_item(host, &item)
                .filter(|bytes| !bytes.is_empty() && bytes.len() <= MAX_ITEM_BYTES)
                .ok_or_else(|| format!("无法编码背包 {id} 的第 {slot} 槽。"))?;
            encoded_bytes += bytes.len();
            if encoded_bytes > MAX_PROFILE_BYTES {
                return Err(OVERSIZE.into());
            }
            slots[slot as usize] = Cow::Owned(bytes);
        }
        profile.backpacks.insert(id, slots);
        if profile.backpacks.len() > MAX_BACKPACKS {
            return Err(INVALID_DATA.into());
        }
        Ok(())
    })?;
    for (&id, slots) in &profile.backpacks {
        for (slot, bytes) in slots.iter().enumerate() {
            if let Cow::Borrowed(bytes) = bytes
                && !bytes.is_empty()
                && !decode_item(host, bytes)
            {
                return Err(format!("无法解码同步档案中背包 {id} 的第 {slot} 槽。"));
            }
        }
    }
    Ok(profile.encode(owner)?)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::{
        backpacks::{Backpack, BackpackId, BackpackStore, GlobalBackpacks},
        items::Kind,
    };
    use mod_api::{HostTransaction, Identifier, Tag, testing::FakeHost};

    const OWNER: u128 = 0x8c0e7dd0005e48a59d4928afb237bc91;
    const PAPER_PROFILE: &[u8] =
        include_bytes!("../tools/paper-storage-export/fixtures/paper-26.2-profile.sfp1");

    fn checksummed(mut payload: Vec<u8>) -> Vec<u8> {
        let digest = Sha256::digest(&payload);
        payload.extend_from_slice(&digest);
        payload
    }

    fn empty_payload() -> Vec<u8> {
        let mut out = b"SFP1".to_vec();
        out.extend_from_slice(&OWNER.to_be_bytes());
        out.push(0);
        out.extend_from_slice(&[0; 12]);
        out
    }

    #[test]
    fn receiver_projects_only_player_persisted_and_refuses_the_wrong_schema() {
        let host = FakeHost::new();
        let candidate = Compound::from([
            (
                "PlayerPersisted".into(),
                Tag::compound([
                    ("slimefun:lumps", Tag::Byte(1)),
                    ("slimefun:other_setting", Tag::Int(4)),
                ]),
            ),
            ("unrelated_root".into(), Tag::Byte(1)),
        ]);
        let bytes = checksummed(empty_payload());
        let entries =
            stage_received_profile(&host, OWNER, &SECTION, 1, &candidate, &bytes).unwrap();
        assert_eq!(
            entries.get(&"slimefun:other_setting".into()),
            Some(&Tag::Int(4))
        );
        assert_eq!(entries.get(&"slimefun:lumps".into()), None);
        assert_eq!(entries.get(&"unrelated_root".into()), None);
        assert!(stage_received_profile(&host, OWNER, &SECTION, 2, &candidate, &bytes).is_err());
        assert!(stage_received_profile(&host, OWNER + 1, &SECTION, 1, &candidate, &bytes).is_err());
    }

    #[test]
    fn source_capture_requires_the_current_owner_and_exports_live_research() {
        let mut host = FakeHost::new();
        let paper = Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        host.transfer_profiles
            .insert((OWNER, (*SECTION).clone()), (41, PAPER_PROFILE.to_vec()));
        assert!(capture_prepared(&host, OWNER, &SECTION, 1).is_err());
        let level = host.level.level();
        let player = host.players.join(OWNER, 1, level);
        player.persistent_data = Compound::from([(
            "PlayerPersisted".into(),
            Tag::compound([("slimefun:advanced_circuit_board", Tag::Byte(1))]),
        )]);
        assert!(capture_prepared(&host, OWNER, &SECTION, 2).is_err());
        let encoded = capture_prepared(&host, OWNER, &SECTION, 1).unwrap();
        let captured = Profile::decode(OWNER, &encoded).unwrap();
        assert_eq!(
            captured.researches,
            BTreeSet::from([
                "addon:research".into(),
                "slimefun:advanced_circuit_board".into()
            ]),
        );
        assert!(captured.legacy_researches.is_empty());
        assert!(captured.guide_issued);
        assert_eq!(captured.backpacks, paper.backpacks);
    }

    #[test]
    fn synchronized_profiles_refuse_wrong_owners_and_incomplete_records() {
        let empty = checksummed(empty_payload());
        assert!(Profile::decode(OWNER, &empty).is_ok());
        assert!(Profile::decode(OWNER + 1, &empty).is_err());
        let mut damaged = empty;
        damaged[20] = 1;
        assert!(Profile::decode(OWNER, &damaged).is_err());

        let mut payload = empty_payload();
        payload[20] = 2;
        assert!(Profile::decode(OWNER, &checksummed(payload)).is_err());

        let mut payload = empty_payload();
        payload[29..33].copy_from_slice(&1u32.to_be_bytes());
        // Nine slots declared, one item claims more bytes than the section contains.
        payload.extend_from_slice(&[0, 0, 0, 14, 0, 0, 0, 9, 0, 0, 0, 4, 1]);
        assert!(Profile::decode(OWNER, &checksummed(payload)).is_err());

        let mut payload = empty_payload();
        payload.push(0);
        assert!(Profile::decode(OWNER, &checksummed(payload)).is_err());
    }

    #[test]
    fn paper_26_2_profile_preserves_identity_research_and_exact_slot_bytes() {
        let profile = Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        assert!(profile.guide_issued);
        assert_eq!(
            profile.researches,
            BTreeSet::from(["addon:research".into(), "slimefun:lumps".into()])
        );
        assert_eq!(profile.legacy_researches, BTreeSet::from([31]));
        assert_eq!(
            profile.slimefun_researches(),
            BTreeSet::from(["slimefun:lumps".into(), "slimefun:cactus_armor".into()])
        );
        assert_eq!(profile.backpacks.keys().copied().collect::<Vec<_>>(), [14]);
        let slots = &profile.backpacks[&14];
        assert_eq!(slots.len(), 9);
        assert_eq!(slots.iter().filter(|item| !item.is_empty()).count(), 1);
        assert_eq!(
            format!("{:x}", Sha256::digest(&slots[7])),
            "0ea0142ea4da499c768ee8a6f57bacfa2b09cad7526b094044e74af9b022426d"
        );
        assert_eq!(profile.encode(OWNER).unwrap(), PAPER_PROFILE);
    }

    #[test]
    fn destination_staging_refuses_wrong_owners_and_a_bad_item_in_the_last_slot() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:diamond").unwrap()])
            .unwrap();
        let data = Compound::new();
        let wrong_owner = stage_apply_with_decoder(&host, OWNER + 1, &data, PAPER_PROFILE, |_| {
            panic!("a profile for another owner must be rejected before decoding items")
        });
        assert!(wrong_owner.unwrap_err().contains("UUID 不匹配"));

        let diamond = Kind::Vanilla("minecraft:diamond").stack(&host, 3).unwrap();
        let mut profile = Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        let mut late_bag = vec![Cow::Borrowed(&[][..]); 9];
        late_bag[8] = Cow::Borrowed(b"invalid last item");
        profile.backpacks.insert(30, late_bag);
        let bytes = profile.encode(OWNER).unwrap();
        let visits = Cell::new(0);
        let failure = stage_apply_with_decoder(&host, OWNER, &data, &bytes, |encoded| {
            let slot = visits.get();
            visits.set(slot + 1);
            if slot == 0 {
                assert_eq!(encoded, profile.backpacks[&14][7].as_ref());
                Some(diamond.clone())
            } else {
                assert_eq!(encoded, b"invalid last item");
                None
            }
        });
        assert_eq!(visits.get(), 2);
        assert!(failure.unwrap_err().contains("背包 30 的第 8 槽"));
        assert!(data.is_empty());

        let accepted = stage_apply_with_decoder(&host, OWNER, &data, PAPER_PROFILE, |_| {
            Some(diamond.clone())
        })
        .unwrap();
        assert!(accepted.guide_issued);
        assert_eq!(
            accepted
                .player_data
                .get(&"PlayerPersisted".into())
                .unwrap()
                .get("slimefun:sync_guide_issued"),
            Some(&Tag::Byte(1))
        );
    }

    #[test]
    fn destination_replacement_erases_stale_owned_state_and_preserves_other_players() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:diamond").unwrap()])
            .unwrap();
        let diamond = Kind::Vanilla("minecraft:diamond").stack(&host, 3).unwrap();
        let gps = Tag::compound([("world", Tag::String("minecraft:the_nether".into()))]);
        let base = Compound::from([
            (
                "PlayerPersisted".into(),
                Tag::compound([
                    ("slimefun:alloys", Tag::Byte(1)),
                    (
                        "slimefun:pending_research",
                        Tag::compound([("step", Tag::Byte(2))]),
                    ),
                    ("slimefun:sync_guide_issued", Tag::Byte(1)),
                    ("addon:preference", Tag::Int(17)),
                ]),
            ),
            ("ServerLocalGPS".into(), gps.clone()),
        ]);
        let mut source = Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        source.guide_issued = false;
        source.legacy_researches.extend([173, -307]);
        source
            .backpacks
            .insert(22, vec![Cow::Borrowed(&[][..]); 18]);
        let bytes = source.encode(OWNER).unwrap();
        let staged = stage_apply_with_decoder(&host, OWNER, &base, &bytes, |encoded| {
            assert_eq!(encoded, source.backpacks[&14][7].as_ref());
            Some(diamond.clone())
        })
        .unwrap();
        assert_eq!(staged.owner, OWNER);
        assert!(!staged.guide_issued);
        assert_eq!(
            staged.unknown_researches,
            BTreeSet::from(["addon:research".into()])
        );
        assert_eq!(staged.unknown_legacy_researches, BTreeSet::from([-307]));
        let saved = staged.player_data.get(&"PlayerPersisted".into()).unwrap();
        for id in [
            "slimefun:lumps",
            "slimefun:cactus_armor",
            "slimefun:bio_reactor",
            "slimefun:coal_generator",
        ] {
            assert_eq!(saved.get(id), Some(&Tag::Byte(1)), "{id}");
        }
        for id in [
            "slimefun:alloys",
            "slimefun:pending_research",
            "slimefun:sync_guide_issued",
            "addon:research",
        ] {
            assert_eq!(saved.get(id), None, "{id}");
        }
        assert_eq!(saved.get("addon:preference"), Some(&Tag::Int(17)));
        assert_eq!(staged.player_data.get(&"ServerLocalGPS".into()), Some(&gps));
        assert_eq!(
            base.get(&"PlayerPersisted".into())
                .unwrap()
                .get("slimefun:alloys"),
            Some(&Tag::Byte(1))
        );

        let key = |owner, number| BackpackId { owner, number }.storage_key();
        let other = key(OWNER + 1, 14);
        let records = Compound::from([
            (key(OWNER, 2).as_str().into(), Tag::Byte(1)),
            (key(OWNER, 14).as_str().into(), Tag::Byte(2)),
            (other.as_str().into(), Tag::Int(23)),
        ]);
        let staged_records = staged.replace_backpack_records(records);
        assert_eq!(staged_records.len(), 3);
        assert_eq!(staged_records.get(&key(OWNER, 2).as_str().into()), None);
        assert_eq!(
            staged_records.get(&other.as_str().into()),
            Some(&Tag::Int(23))
        );
        let load = |number| {
            let Some(Tag::Compound(record)) =
                staged_records.get(&key(OWNER, number).as_str().into())
            else {
                panic!("missing staged backpack {number}");
            };
            assert_eq!(record.get(&"paper".into()), Some(&Tag::Byte(0)));
            Backpack::from_nbt(&host, record).unwrap()
        };
        assert_eq!(load(14).slots, BTreeMap::from([(7, diamond)]));
        let empty = load(22);
        assert_eq!(empty.size, 18);
        assert!(empty.slots.is_empty());
    }

    #[test]
    fn the_writer_bounds_the_key_bag_and_section_before_appending_a_digest() {
        let mut profile = Profile::default();
        profile.researches.insert("addon:研究".into());
        assert_eq!(profile.encode(OWNER), Err(INVALID_DATA));
        profile.researches.clear();
        let empty = Cow::Borrowed(&[][..]);
        profile.backpacks.insert(0, vec![empty.clone(); 8]);
        assert_eq!(profile.encode(OWNER), Err(INVALID_DATA));
        profile.backpacks.clear();
        profile
            .backpacks
            .insert(i32::MAX as u32 + 1, vec![empty; 9]);
        assert_eq!(profile.encode(OWNER), Err(INVALID_DATA));
        profile.backpacks.clear();
        let full_slot = vec![1; MAX_ITEM_BYTES];
        profile
            .backpacks
            .insert(0, vec![Cow::Borrowed(&full_slot); 9]);
        assert_eq!(profile.encode(OWNER), Err(OVERSIZE));
    }

    fn remember_live_bag(host: &mut FakeHost, owner: u128, number: u32, slots: Compound) {
        let data = Compound::from([
            ("size".into(), Tag::Int(9)),
            ("slots".into(), Tag::Compound(slots)),
            ("paper".into(), Tag::Byte(0)),
        ]);
        host.transaction(|host, tx| {
            GlobalBackpacks.save(host, tx, BackpackId { owner, number }, data)
        })
        .unwrap();
    }

    #[test]
    fn prepared_capture_preserves_unknown_research_and_uses_live_bags_including_empty_ones() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:diamond").unwrap()])
            .unwrap();
        let diamond = Kind::Vanilla("minecraft:diamond").stack(&host, 3).unwrap();
        let slots = Compound::from([("2".into(), Tag::Compound(diamond.to_nbt(&host).unwrap()))]);
        remember_live_bag(&mut host, OWNER + 1, 13, slots.clone());
        remember_live_bag(&mut host, OWNER, 14, Compound::new());
        remember_live_bag(&mut host, OWNER, 22, slots);
        let mut restored = Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        let original_item = restored.backpacks[&14][7].clone();
        restored
            .backpacks
            .insert(18, restored.backpacks[&14].clone());
        restored.legacy_researches.insert(-307);
        restored.guide_issued = false;
        let data = Compound::from([(
            "PlayerPersisted".into(),
            Tag::compound([
                ("slimefun:cactus_armor", Tag::Byte(1)),
                ("slimefun:advanced_circuit_board", Tag::Byte(1)),
                ("another-mod:setting", Tag::Byte(1)),
            ]),
        )]);
        let output = capture_state(
            &host,
            OWNER,
            &data,
            restored,
            CaptureBags::Legacy(None),
            |_, bytes| bytes == original_item.as_ref(),
            |_, stack| (*stack == diamond).then(|| original_item.to_vec()),
        )
        .unwrap();
        let profile = Profile::decode(OWNER, &output).unwrap();
        assert!(profile.guide_issued);
        assert_eq!(
            profile.researches,
            BTreeSet::from([
                "addon:research".into(),
                "slimefun:advanced_circuit_board".into(),
                "slimefun:cactus_armor".into(),
            ])
        );
        assert_eq!(profile.legacy_researches, BTreeSet::from([-307]));
        assert_eq!(
            profile.backpacks.keys().copied().collect::<Vec<_>>(),
            [14, 18, 22]
        );
        assert!(profile.backpacks[&14].iter().all(|slot| slot.is_empty()));
        assert_eq!(profile.backpacks[&18][7], original_item);
        assert_eq!(profile.backpacks[&22][2], original_item);
        assert_eq!(
            profile.backpacks[&22]
                .iter()
                .filter(|slot| !slot.is_empty())
                .count(),
            1
        );
    }

    #[test]
    fn capture_uses_only_the_committed_baseline_and_its_live_bag_overrides() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:diamond").unwrap()])
            .unwrap();
        let diamond = Kind::Vanilla("minecraft:diamond").stack(&host, 3).unwrap();
        let contents =
            Compound::from([("2".into(), Tag::Compound(diamond.to_nbt(&host).unwrap()))]);
        remember_live_bag(&mut host, OWNER, 77, contents.clone());
        let mut baseline = Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        let paper_item = baseline.backpacks[&14][7].to_vec();
        baseline
            .backpacks
            .insert(18, baseline.backpacks[&14].clone());
        host.transfer_profiles.insert(
            (OWNER, (*SECTION).clone()),
            (41, baseline.encode(OWNER).unwrap()),
        );
        remember_live_bag(&mut host, OWNER, 14, Compound::new());
        remember_live_bag(&mut host, OWNER, 19, contents);

        let output = capture_state(
            &host,
            OWNER,
            &Compound::new(),
            baseline,
            CaptureBags::Committed(41),
            |_, bytes| bytes == paper_item,
            |_, stack| (*stack == diamond).then(|| paper_item.clone()),
        )
        .unwrap();
        let profile = Profile::decode(OWNER, &output).unwrap();
        assert_eq!(
            profile.backpacks.keys().copied().collect::<Vec<_>>(),
            [14, 18, 19]
        );
        assert!(profile.backpacks[&14].iter().all(|slot| slot.is_empty()));
        assert_eq!(profile.backpacks[&18][7].as_ref(), paper_item);
        assert_eq!(profile.backpacks[&19][2].as_ref(), paper_item);
        assert!(profile.researches.contains("addon:research"));
    }

    #[test]
    fn pending_paid_research_and_unreadable_archived_items_cannot_be_captured() {
        let host = FakeHost::new();
        let pending = Compound::from([(
            "PlayerPersisted".into(),
            Tag::compound([(
                "slimefun:pending_research",
                Tag::compound([("id", Tag::String("lumps".into())), ("step", Tag::Byte(2))]),
            )]),
        )]);
        let archived = || Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        let pending_result = capture_state(
            &host,
            OWNER,
            &pending,
            archived(),
            CaptureBags::Legacy(None),
            |_, _| panic!("pending research must be refused before reading the archived items"),
            |_, _| unreachable!(),
        );
        assert!(pending_result.unwrap_err().contains("已付费的研究"));
        let invalid_bag = capture_state(
            &host,
            OWNER,
            &Compound::new(),
            archived(),
            CaptureBags::Legacy(None),
            |_, _| false,
            |_, _| unreachable!(),
        );
        assert!(invalid_bag.unwrap_err().contains("背包 14 的第 7 槽"));
    }

    #[test]
    fn a_normalized_paper_profile_is_the_bag_baseline_when_present() {
        let host = FakeHost::new();
        let normalized = format!(
            "format = 1\nplayer = '{OWNER:032x}'\npaper_profile = 'original'\n\
             [[backpacks]]\nid = 31\nsize = 9\ninventory_slots = 0\npaper_items = []\n"
        )
        .parse::<toml::Table>()
        .unwrap();
        let archived = Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        let output = capture_state(
            &host,
            OWNER,
            &Compound::new(),
            archived,
            CaptureBags::Legacy(Some(&normalized)),
            |_, _| panic!("the normalized profile has precedence over the older raw bags"),
            |_, _| unreachable!(),
        )
        .unwrap();
        let profile = Profile::decode(OWNER, &output).unwrap();
        assert_eq!(profile.backpacks.keys().copied().collect::<Vec<_>>(), [31]);
        assert_eq!(
            profile.researches,
            BTreeSet::from(["addon:research".into()])
        );
    }
}
