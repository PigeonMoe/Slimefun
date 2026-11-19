// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeSet, fmt::Write as _, sync::Mutex};

use mod_api::{ConfigFileError, ConfigPath, HostTransaction, ServerHost};

use crate::{
    android_program::{Mode, Script},
    gps_waypoints::quote,
};

const MAX_FILE: usize = 16 * 1024 * 1024;
const MAX_NAME: usize = 1024;
const LIVE_FILE: &str = "android-scripts.toml";
const PAPER_FILE: &str = "android-scripts-import.toml";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub name: String,
    pub author: u128,
    pub author_name: String,
    pub code: String,
    pub downloads: u32,
    pub positive: BTreeSet<u128>,
    pub negative: BTreeSet<u128>,
    category: String,
}

impl Entry {
    pub fn can_rate(&self, player: u128) -> bool {
        player != self.author
            && !self.positive.contains(&player)
            && !self.negative.contains(&player)
    }

    fn vote(&mut self, player: u128, positive: bool) -> Result<(), String> {
        if player == self.author {
            return Err("You cannot rate your own script.".into());
        }
        if !self.can_rate(player) {
            return Err("You already rated this script.".into());
        }
        if positive {
            self.positive.insert(player);
        } else {
            self.negative.insert(player);
        }
        Ok(())
    }

    pub fn rating(&self) -> f32 {
        let positive = (self.positive.len() + 1) as f32;
        (100.0 * (positive / (positive + self.negative.len() as f32))).round() / 100.0
    }
}

#[derive(Debug, Default)]
pub(crate) struct Entries {
    entries: Vec<Entry>,
    imported: bool,
}

impl Entries {
    pub fn available(&self, mode: Mode) -> Vec<usize> {
        let Some(category) = category(mode) else {
            return Vec::new();
        };
        let mut matches: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.category == "NONE" || entry.category == category)
            .collect();
        matches.sort_by_key(|(_, entry)| {
            (
                std::cmp::Reverse(entry.positive.len() + entry.negative.len()),
                entry.category == "NONE",
            )
        });
        matches.into_iter().map(|(index, _)| index).collect()
    }

    pub fn get(&self, id: usize) -> Option<&Entry> {
        self.entries.get(id)
    }

    pub fn has_code(&self, mode: Mode, code: &str) -> bool {
        self.available(mode)
            .iter()
            .any(|&id| self.entries[id].code == code)
    }

    pub fn upload(
        &mut self,
        mode: Mode,
        author: u128,
        author_name: &str,
        title: &str,
        script: &Script,
    ) -> Result<(), String> {
        let category = category(mode).ok_or("This Android type does not share scripts yet.")?;
        let name = clean_name(title).ok_or("Enter a script name of at most 1024 bytes.")?;
        if !valid_text(author_name) {
            return Err("The script author's name is unavailable.".into());
        }
        let code = script.source();
        if !script.can_run(mode) {
            return Err("This Android cannot run all of the instructions in this script.".into());
        }
        if self.has_code(mode, &code) {
            return Err("This script was already uploaded.".into());
        }
        self.entries.push(Entry {
            name,
            author,
            author_name: author_name.into(),
            code,
            downloads: 0,
            positive: BTreeSet::new(),
            negative: BTreeSet::new(),
            category: category.into(),
        });
        Ok(())
    }

    pub fn rate(&mut self, id: usize, player: u128, positive: bool) -> Result<(), String> {
        self.entries
            .get_mut(id)
            .ok_or("This script no longer exists.")?
            .vote(player, positive)
    }

    pub fn download(&mut self, id: usize, mode: Mode) -> Result<Script, String> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or("This script no longer exists.")?;
        let script = Script::parse(&entry.code)
            .filter(|script| script.can_run(mode))
            .ok_or("This script contains instructions that this Android cannot run.")?;
        entry.downloads = entry.downloads.saturating_add(1);
        Ok(script)
    }
}

fn category(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::Normal => Some("NONE"),
        Mode::Farmer => Some("FARMER"),
        Mode::Archived => None,
    }
}

fn valid_text(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= MAX_NAME && !text.chars().any(char::is_control)
}

fn clean_name(text: &str) -> Option<String> {
    let mut result = String::new();
    let mut input = text.chars().peekable();
    while let Some(ch) = input.next() {
        if matches!(ch, '&' | '§') && input.peek().is_some_and(|ch| {
            matches!(ch.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r' | 'x')
        }) {
            input.next();
        } else {
            result.push(ch);
        }
    }
    valid_text(&result).then_some(result)
}

#[derive(Default)]
pub(crate) struct Library(Mutex<()>);

impl Library {
    pub fn load(&self, host: &dyn ServerHost) -> Result<Entries, String> {
        let _lock = self.0.lock().unwrap();
        load(host)
    }

    pub fn update<T>(
        &self,
        host: &mut dyn ServerHost,
        f: impl FnOnce(&mut dyn ServerHost, &mut Entries) -> Result<T, String>,
    ) -> Result<T, String> {
        let _lock = self.0.lock().unwrap();
        // A download updates the device and the registry together. A failed
        // registry write rolls back the attachment write through the host.
        host.transaction(|host, _| {
            let mut entries = load(host)?;
            let result = f(host, &mut entries)?;
            save(host, &entries)?;
            Ok(result)
        })
    }
}

fn load(host: &dyn ServerHost) -> Result<Entries, String> {
    let mut entries = read(host, LIVE_FILE)?.unwrap_or_default();
    if !entries.imported
        && let Some(archive) = read(host, PAPER_FILE)?
    {
        entries.entries.extend(archive.entries);
        entries.imported = true;
    }
    Ok(entries)
}

fn read(host: &dyn ServerHost, name: &str) -> Result<Option<Entries>, String> {
    match host
        .world_data_files()
        .read(&ConfigPath::new(name).expect("fixed script path"))
    {
        Ok(bytes) => parse(&bytes).map(Some),
        Err(ConfigFileError::NotFound) => Ok(None),
        Err(error) => Err(format!(
            "Could not read the Android script library: {error}"
        )),
    }
}

fn save(host: &dyn ServerHost, entries: &Entries) -> Result<(), String> {
    let bytes = encode(entries);
    if bytes.len() > MAX_FILE {
        return Err("The Android script library is full.".into());
    }
    host.world_data_files()
        .write(
            &ConfigPath::new(LIVE_FILE).expect("fixed script path"),
            bytes.as_bytes(),
        )
        .map_err(|error| format!("Could not save the Android script library: {error}"))
}

fn parse(bytes: &[u8]) -> Result<Entries, String> {
    let table = std::str::from_utf8(bytes)
        .ok()
        .filter(|text| text.len() <= MAX_FILE)
        .and_then(|text| text.parse::<toml::Table>().ok())
        .filter(|table| table.get("format").and_then(toml::Value::as_integer) == Some(1))
        .ok_or("Invalid Android script library.")?;
    let imported = table
        .get("paper_imported")
        .map_or(Some(false), toml::Value::as_bool)
        .ok_or("Invalid Android script import status.")?;
    let rows = table
        .get("scripts")
        .map(|rows| rows.as_array().ok_or("Invalid Android script list."))
        .transpose()?;
    let mut entries = Vec::new();
    for row in rows.into_iter().flatten() {
        entries.push(parse_entry(row).ok_or("Invalid Android script entry.")?);
    }
    Ok(Entries { entries, imported })
}

fn parse_entry(row: &toml::Value) -> Option<Entry> {
    let row = row.as_table()?;
    let field = |name| row.get(name)?.as_str();
    let votes = |name| -> Option<BTreeSet<u128>> {
        let mut players = BTreeSet::new();
        for uuid in row.get(name)?.as_array()? {
            players.insert(parse_uuid(uuid.as_str()?)?);
        }
        Some(players)
    };
    let (name, author_name, category, code) = (
        field("name")?,
        field("author_name")?,
        field("category")?,
        field("code")?,
    );
    if !valid_text(name)
        || !valid_text(author_name)
        || !matches!(
            category,
            "NONE"
                | "MINER"
                | "FARMER"
                | "ADVANCED_FARMER"
                | "WOODCUTTER"
                | "FIGHTER"
                | "FISHERMAN"
                | "NON_FIGHTER"
        )
        || code.is_empty()
        || code.len() > 64 * 1024
        || code.chars().any(char::is_control)
    {
        return None;
    }
    Some(Entry {
        name: name.into(),
        author: parse_uuid(field("author")?)?,
        author_name: author_name.into(),
        code: code.into(),
        downloads: u32::try_from(row.get("downloads")?.as_integer()?).ok()?,
        positive: votes("positive")?,
        negative: votes("negative")?,
        category: category.into(),
    })
}

fn parse_uuid(text: &str) -> Option<u128> {
    (text.len() == 32)
        .then(|| u128::from_str_radix(text, 16).ok())
        .flatten()
}

fn encode(library: &Entries) -> String {
    let mut text = format!("format = 1\npaper_imported = {}\n", library.imported);
    for entry in &library.entries {
        let author = entry.author;
        let downloads = entry.downloads;
        writeln!(&mut text, "\n[[scripts]]\nname = {}\nauthor = \"{author:032x}\"\nauthor_name = {}\ncategory = {}\ncode = {}\ndownloads = {downloads}", quote(&entry.name), quote(&entry.author_name), quote(&entry.category), quote(&entry.code)).unwrap();
        for (label, voting) in [("positive", &entry.positive), ("negative", &entry.negative)] {
            let votes = voting
                .iter()
                .map(|id| format!("\"{id:032x}\""))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(&mut text, "{label} = [{votes}]").unwrap();
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_shares_normal_scripts_and_keeps_paper_vote_rules() {
        let mut library = Entries::default();
        let normal = Script::default();
        let farmer = Script::parse("START-FARM_FORWARD-REPEAT").unwrap();
        library
            .upload(Mode::Normal, 1, "Juniper", "&aΩ", &normal)
            .unwrap();
        library
            .upload(Mode::Farmer, 2, "Fern", "Harvest", &farmer)
            .unwrap();
        assert!(
            library
                .upload(Mode::Farmer, 2, "Fern", "Duplicate", &normal)
                .is_err()
        );
        assert_eq!(library.available(Mode::Farmer), vec![1, 0]);
        assert_eq!(library.available(Mode::Normal), vec![0]);
        assert_eq!(library.get(0).unwrap().name, "Ω");
        assert!(library.rate(0, 1, false).is_err());
        library.rate(0, 2, false).unwrap();
        assert!(library.rate(0, 2, true).is_err());
        assert_eq!(library.available(Mode::Farmer), vec![0, 1]);
        assert_eq!(library.get(0).unwrap().rating(), 0.5);
        assert!(library.download(1, Mode::Normal).is_err());
        assert_eq!(library.download(1, Mode::Farmer).unwrap(), farmer);
        let restored = parse(encode(&library).as_bytes()).unwrap();
        assert_eq!(restored.get(0), library.get(0));
        assert_eq!(restored.get(1).unwrap().downloads, 1);
    }

    #[test]
    fn an_old_fisherman_script_remains_archived_without_appearing_on_other_devices() {
        let imported = br#"
format = 1
[[scripts]]
name = "Zed's Catch"
author = "0000000000000000000000000000000f"
author_name = "Zed"
category = "FISHERMAN"
code = "START-FISH-REPEAT"
downloads = 21
positive = ["00000000000000000000000000000001"]
negative = []
"#;
        let entries = parse(imported).unwrap();
        assert!(entries.available(Mode::Normal).is_empty());
        assert!(entries.available(Mode::Farmer).is_empty());
        let entry = entries.get(0).unwrap();
        assert_eq!(entry.downloads, 21);
        assert_eq!(entry.code, "START-FISH-REPEAT");
        assert_eq!(entry.author, 15);
        assert_eq!(
            parse(encode(&entries).as_bytes()).unwrap().get(0),
            Some(entry)
        );
    }
}
