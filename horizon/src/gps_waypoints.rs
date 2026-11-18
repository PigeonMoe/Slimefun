// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, fmt::Write as _};

use mod_api::{ConfigFileError, ConfigPath, Identifier, ServerHost};

pub(crate) const MAX_PLAYER_WAYPOINTS: usize = 20;
const MAX_FILE: usize = 512 * 1024;
const MAX_RECORDS: usize = 1024;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Location {
    pub dimension: String,
    pub position: [f64; 3],
    pub rotation: [f32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Waypoint {
    pub name: String,
    pub location: Location,
}

#[derive(Default)]
pub(crate) struct Waypoints {
    pub entries: BTreeMap<String, Waypoint>,
    imported: bool,
}

pub(crate) fn point_id(name: &str) -> Option<String> {
    let mut letters = name.chars().peekable();
    let mut id = String::new();
    while let Some(letter) = letters.next() {
        if matches!(letter, '&' | '§')
            && letters.peek().is_some_and(|value| {
                matches!(value.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r' | 'x')
            })
        {
            letters.next();
        } else if letter == ' ' {
            id.push('_');
        } else {
            id.extend(letter.to_uppercase());
        }
    }
    (!name.trim().is_empty() && valid_name(&id) && valid_name(name)).then_some(id)
}

fn valid_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}

impl Location {
    fn valid(&self) -> bool {
        Identifier::parse(&self.dimension).is_ok()
            && self.rotation.iter().all(|angle| angle.is_finite())
            && self
                .position
                .iter()
                .zip([30_000_000.0, 20_000_000.0, 30_000_000.0])
                .all(|(&value, limit)| (-limit..limit).contains(&value))
    }
}

fn path(owner: u128, archive: bool) -> ConfigPath {
    let directory = if archive {
        "waypoints-import"
    } else {
        "waypoints"
    };
    ConfigPath::new(&format!("{directory}/{owner:032x}.toml")).unwrap()
}

fn read(host: &dyn ServerHost, owner: u128, archive: bool) -> Result<Option<Waypoints>, String> {
    match host.world_data_files().read(&path(owner, archive)) {
        Ok(bytes) => parse(owner, &bytes).map(Some),
        Err(ConfigFileError::NotFound) => Ok(None),
        Err(error) => Err(format!("Could not read the GPS waypoints: {error}")),
    }
}

pub(crate) fn load(host: &dyn ServerHost, owner: u128) -> Result<Waypoints, String> {
    read(host, owner, false).map(Option::unwrap_or_default)
}

pub(crate) fn save(host: &dyn ServerHost, owner: u128, points: &Waypoints) -> Result<(), String> {
    let text = encode(owner, points);
    if text.len() > MAX_FILE || points.entries.len() > MAX_RECORDS {
        return Err("The GPS waypoint store is full.".into());
    }
    host.world_data_files()
        .write(&path(owner, false), text.as_bytes())
        .map_err(|error| format!("Could not save the GPS waypoints: {error}"))
}

pub(crate) fn import(host: &dyn ServerHost, owner: u128) -> Result<(), String> {
    let mut current = load(host, owner)?;
    if current.imported {
        return Ok(());
    }
    if let Some(archive) = read(host, owner, true)? {
        current.merge_once(archive);
        save(host, owner, &current)?;
    }
    Ok(())
}

impl Waypoints {
    fn merge_once(&mut self, archive: Self) {
        if self.imported {
            return;
        }
        for (id, point) in archive.entries {
            self.entries.entry(id).or_insert(point);
        }
        self.imported = true;
    }
}

fn number(value: &toml::Value) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|value| value as f64))
}

fn parse(owner: u128, bytes: &[u8]) -> Result<Waypoints, String> {
    if bytes.len() > MAX_FILE {
        return Err("The GPS waypoint file is too large.".into());
    }
    let table = std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .filter(|table| table.get("format").and_then(toml::Value::as_integer) == Some(1))
        .ok_or("Invalid GPS waypoint file.")?;
    let expected_owner = format!("{owner:032x}");
    if table.get("owner").and_then(toml::Value::as_str) != Some(expected_owner.as_str()) {
        return Err("The GPS waypoint file belongs to another player.".into());
    }
    let imported = table
        .get("paper_imported")
        .map_or(Some(false), toml::Value::as_bool)
        .ok_or("Invalid GPS import status.")?;
    let mut result = Waypoints {
        entries: BTreeMap::new(),
        imported,
    };
    let records = table
        .get("waypoints")
        .map(|rows| rows.as_array().ok_or("Invalid GPS waypoint list."))
        .transpose()?;
    for row in records.into_iter().flatten() {
        let (id, point) = parse_point(row).ok_or("Invalid GPS waypoint.")?;
        if result.entries.len() >= MAX_RECORDS || result.entries.insert(id, point).is_some() {
            return Err("The GPS waypoint file has too many entries or duplicate IDs.".into());
        }
    }
    Ok(result)
}

fn parse_point(value: &toml::Value) -> Option<(String, Waypoint)> {
    let row = value.as_table()?;
    let string = |field| row.get(field)?.as_str();
    let id = string("id").filter(|name| valid_name(name))?.to_owned();
    let name = string("name").filter(|name| valid_name(name))?.to_owned();
    let dimension = string("dimension")?.to_owned();
    let [x, y, z] = row.get("position")?.as_array()?.as_slice() else {
        return None;
    };
    let [yaw, pitch] = row.get("rotation")?.as_array()?.as_slice() else {
        return None;
    };
    let location = Location {
        dimension,
        position: [number(x)?, number(y)?, number(z)?],
        rotation: [number(yaw)? as f32, number(pitch)? as f32],
    };
    location
        .valid()
        .then_some((id, Waypoint { name, location }))
}

fn encode(owner: u128, rules: &Waypoints) -> String {
    let mut text = format!(
        "format = 1\nowner = \"{owner:032x}\"\npaper_imported = {}\n",
        rules.imported
    );
    for (id, point) in &rules.entries {
        let Location {
            dimension,
            position,
            rotation,
        } = &point.location;
        writeln!(
            text,
            "\n[[waypoints]]\nid = {}\nname = {}\ndimension = {}\nposition = {position:?}\nrotation = {rotation:?}",
            quote(id), quote(&point.name), quote(dimension),
        ).unwrap();
    }
    text
}

fn quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waypoint_ids_keep_paper_unicode_uppercase_and_color_stripping() {
        assert_eq!(point_id("&aStraße §b家").as_deref(), Some("STRASSE_家"));
        assert_eq!(point_id("Mine  Exit").as_deref(), Some("MINE__EXIT"));
        assert_eq!(point_id("&a"), None);
        assert_eq!(point_id("\n"), None);
    }

    #[test]
    fn waypoint_archive_keeps_coordinates_owner_and_replay_safety() {
        let point = Waypoint {
            name: "家 \"Queens\\Road\"".into(),
            location: Location {
                dimension: "minecraft:the_nether".into(),
                position: [-18.75, 64.0, -0.25],
                rotation: [66.0, -4.0],
            },
        };
        let mut archive = Waypoints::default();
        archive.entries.insert("家".into(), point.clone());
        let bytes = encode(31, &archive);
        assert_eq!(
            parse(31, bytes.as_bytes()).unwrap().entries.get("家"),
            Some(&point)
        );
        assert!(parse(32, bytes.as_bytes()).is_err());
        let mut current = Waypoints::default();
        current.merge_once(archive);
        assert_eq!(current.entries.remove("家"), Some(point));
        let mut reread = parse(31, encode(31, &current).as_bytes()).unwrap();
        reread.merge_once(parse(31, bytes.as_bytes()).unwrap());
        assert!(reread.entries.is_empty());
    }
}
