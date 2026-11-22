// SPDX-License-Identifier: GPL-3.0-only

//! The Paper Slimefun section of a PM-Sync snapshot (SFP1).

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
};

use mod_api::{Compound, ConfigFileError, ConfigPath, ItemStack, PlayerRef, ServerHost};
use sha2::{Digest, Sha256};

use crate::{backpacks, research};

const MAX_PROFILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ITEM_BYTES: usize = 1024 * 1024;
const MAX_RESEARCHES: usize = 16_384;
const MAX_RESEARCH_KEY_BYTES: usize = 512;
const MAX_BACKPACKS: usize = 4_096;
const DIGEST_BYTES: usize = 32;
const OVERSIZE: &str = "Slimefun 同步档案大小超出允许范围。";
const INVALID_DATA: &str = "Slimefun 同步档案包含无效数据。";
const TRUNCATED: &str = "Slimefun 同步档案不完整。";

#[derive(Default)]
pub(crate) struct Profile<'a> {
    pub(crate) guide_issued: bool,
    pub(crate) researches: BTreeSet<String>,
    pub(crate) legacy_researches: BTreeSet<i32>,
    /// Number → every slot in original order. Empty bytes denote an empty slot.
    pub(crate) backpacks: BTreeMap<u32, Vec<Cow<'a, [u8]>>>,
}

impl<'a> Profile<'a> {
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

/// Take the Slimefun section after the coordinator has settled the first-guide
/// lifecycle and prepared the current online player for transfer. The caller
/// holds the interaction lock and keeps it through capture of other sections.
pub(crate) fn capture_prepared(
    host: &mut dyn ServerHost,
    player: PlayerRef,
) -> Result<Vec<u8>, String> {
    let owner = player.uuid();
    if host.player(owner) != Some(player) {
        return Err("只能采集当前在线会话的 Slimefun 档案。".into());
    }
    let data = player
        .persistent_data(host)
        .ok_or("无法读取当前玩家的研究档案。")?;
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
        paper.as_ref(),
        |host, bytes| host.item_from_saved_gzip_nbt(bytes.to_vec()).is_some(),
        |host, item| host.item_to_saved_gzip_nbt(item),
    )
}

fn capture_state(
    host: &mut dyn ServerHost,
    owner: u128,
    data: &Compound,
    mut profile: Profile<'_>,
    paper: Option<&toml::Table>,
    decode_item: impl Fn(&dyn ServerHost, &[u8]) -> bool,
    encode_item: impl Fn(&dyn ServerHost, &ItemStack) -> Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    research::update_capture(data, &mut profile)?;
    if paper.is_some() {
        // Match lazy restoration: a normalized Paper profile takes precedence
        // over the SFP1 archive for bags, and live records win for both formats.
        profile.backpacks.clear();
    }
    let mut encoded_bytes = 0;
    backpacks::visit_snapshot_bags(host, owner, paper, MAX_BACKPACKS, |host, id, bag| {
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
    use super::*;
    use crate::{
        backpacks::{BackpackId, BackpackStore, GlobalBackpacks},
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
            &mut host,
            OWNER,
            &data,
            restored,
            None,
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
    fn pending_paid_research_and_unreadable_archived_items_cannot_be_captured() {
        let mut host = FakeHost::new();
        let pending = Compound::from([(
            "PlayerPersisted".into(),
            Tag::compound([(
                "slimefun:pending_research",
                Tag::compound([("id", Tag::String("lumps".into())), ("step", Tag::Byte(2))]),
            )]),
        )]);
        let archived = || Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        let pending_result = capture_state(
            &mut host,
            OWNER,
            &pending,
            archived(),
            None,
            |_, _| panic!("pending research must be refused before reading the archived items"),
            |_, _| unreachable!(),
        );
        assert!(pending_result.unwrap_err().contains("已付费的研究"));
        let invalid_bag = capture_state(
            &mut host,
            OWNER,
            &Compound::new(),
            archived(),
            None,
            |_, _| false,
            |_, _| unreachable!(),
        );
        assert!(invalid_bag.unwrap_err().contains("背包 14 的第 7 槽"));
    }

    #[test]
    fn a_normalized_paper_profile_is_the_bag_baseline_when_present() {
        let mut host = FakeHost::new();
        let normalized = format!(
            "format = 1\nplayer = '{OWNER:032x}'\npaper_profile = 'original'\n\
             [[backpacks]]\nid = 31\nsize = 9\ninventory_slots = 0\npaper_items = []\n"
        )
        .parse::<toml::Table>()
        .unwrap();
        let archived = Profile::decode(OWNER, PAPER_PROFILE).unwrap();
        let output = capture_state(
            &mut host,
            OWNER,
            &Compound::new(),
            archived,
            Some(&normalized),
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
