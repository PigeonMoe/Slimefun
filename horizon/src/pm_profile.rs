// SPDX-License-Identifier: GPL-3.0-only

//! The Paper Slimefun section of a PM-Sync snapshot (SFP1).

use std::collections::{BTreeMap, BTreeSet};

use mod_api::{ConfigFileError, ConfigPath, ServerHost};
use sha2::{Digest, Sha256};

const MAX_PROFILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ITEM_BYTES: usize = 1024 * 1024;
const MAX_RESEARCHES: usize = 16_384;
const MAX_BACKPACKS: usize = 4_096;
const DIGEST_BYTES: usize = 32;
const INVALID_DATA: &str = "Slimefun 同步档案包含无效数据。";
const TRUNCATED: &str = "Slimefun 同步档案不完整。";

pub(crate) struct Profile<'a> {
    pub(crate) guide_issued: bool,
    pub(crate) researches: BTreeSet<String>,
    pub(crate) legacy_researches: BTreeSet<i32>,
    /// Number → every slot in original order. Empty bytes denote an empty slot.
    pub(crate) backpacks: BTreeMap<u32, Vec<&'a [u8]>>,
}

impl<'a> Profile<'a> {
    /// Match Paper's registered keys and each of the IDs shared by multiple
    /// Research definitions. Unknown add-on values remain in the source file.
    pub(crate) fn slimefun_researches(&self) -> BTreeSet<String> {
        include_str!("knowledge_researches.tsv")
            .lines()
            .filter_map(|row| {
                let (key, rest) = row.split_once('\t')?;
                let (_, legacy) = rest.rsplit_once('\t')?;
                let legacy = legacy.parse().ok()?;
                let key = format!("slimefun:{key}");
                (self.researches.contains(&key) || self.legacy_researches.contains(&legacy))
                    .then_some(key)
            })
            .collect()
    }

    pub(crate) fn decode(owner: u128, bytes: &'a [u8]) -> Result<Self, &'static str> {
        if !(64..=MAX_PROFILE_BYTES).contains(&bytes.len()) {
            return Err("Slimefun 同步档案大小超出允许范围。");
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
                slots.push(input.take(length)?);
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
        let mut key = String::with_capacity(usize::from(length).min(512));
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
            if unit > 127 || key.len() == 512 {
                return Err(INVALID_DATA);
            }
            key.push(char::from(unit as u8));
        }
        let valid = |part: &str, allow_slash: bool| {
            !part.is_empty()
                && part.bytes().all(|byte| {
                    matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-')
                        || allow_slash && byte == b'/'
                })
        };
        if key
            .split_once(':')
            .is_none_or(|(namespace, path)| !valid(namespace, false) || !valid(path, true))
        {
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

pub(crate) fn with_archive<T>(
    host: &dyn ServerHost,
    owner: u128,
    read: impl FnOnce(Profile<'_>) -> Result<T, String>,
) -> Result<Option<T>, String> {
    let path = ConfigPath::new(&format!("pm-sync-import/{owner:032x}.sfp1"))
        .expect("a hexadecimal UUID is a valid archive path");
    let bytes = match host.world_data_files().read(&path) {
        Ok(bytes) => bytes,
        Err(ConfigFileError::NotFound) => return Ok(None),
        Err(error) => return Err(format!("无法读取 Slimefun 同步档案。{error}")),
    };
    read(Profile::decode(owner, &bytes)?).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: u128 = 0x8c0e7dd0005e48a59d4928afb237bc91;

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
        let fixture =
            include_bytes!("../tools/paper-storage-export/fixtures/paper-26.2-profile.sfp1");
        let profile = Profile::decode(OWNER, fixture).unwrap();
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
            format!("{:x}", Sha256::digest(slots[7])),
            "0ea0142ea4da499c768ee8a6f57bacfa2b09cad7526b094044e74af9b022426d"
        );
    }
}
