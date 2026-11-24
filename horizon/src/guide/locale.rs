// SPDX-License-Identifier: GPL-3.0-only

use std::sync::LazyLock;

use mod_api::{Compound, Tag};
use serde_json::Value;

use super::colored_lore;

static ZH_CN: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("zh-cn.json")).expect("generated simplified Chinese guide")
});

static LOCALES: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("locales.json")).expect("generated stock guide locales")
});

static COMMANDS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("command-locales.json")).expect("generated stock commands")
});

const KEY: &str = "slimefun:language";
const PERSISTED: &str = "PlayerPersisted";

#[derive(Clone, Copy, Debug)]
pub(crate) struct Language {
    code: &'static str,
    fallback: Option<&'static str>,
    server_default: bool,
}

impl PartialEq for Language {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
    }
}

impl Eq for Language {}

impl Language {
    pub(crate) const ENGLISH: Self = Self::new("en");
    pub(crate) const CHINESE: Self = Self::new("zh-CN");

    const fn new(code: &'static str) -> Self {
        Self {
            code,
            fallback: None,
            server_default: false,
        }
    }

    pub(super) fn all() -> impl Iterator<Item = Self> {
        LOCALES["order"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(Self::new)
    }

    fn from_locale(value: &str) -> Option<Self> {
        let code = normalize(value);
        Self::all().find(|language| language.code == code)
    }

    fn configured(value: &str) -> Option<Self> {
        let code = normalize(value);
        LOCALES["packs"]
            .as_object()?
            .get_key_value(&code)
            .map(|(key, _)| Self::new(key))
    }

    pub(super) fn is_server_default(self) -> bool {
        self.server_default
    }

    fn pack(self) -> &'static Value {
        &LOCALES["packs"][self.code]
    }

    fn value(self, keys: &[&str]) -> Option<&'static Value> {
        fn descend<'a>(keys: &[&str], data: &'a Value) -> Option<&'a Value> {
            keys.iter().try_fold(data, |value, key| value.get(*key))
        }
        descend(keys, self.pack())
            .filter(|value| !value.is_null())
            .or_else(|| {
                let fallback = self.fallback?;
                match keys.first().copied()? {
                    "guide" | "languages" | "research-messages" => {
                        descend(keys, Self::new(fallback).pack()).filter(|value| !value.is_null())
                    }
                    _ => None,
                }
            })
            .or_else(|| descend(keys, Self::ENGLISH.pack()))
    }

    pub(crate) fn command_message(self, key: &str) -> &'static str {
        COMMANDS[self.code][key]
            .as_str()
            .or_else(|| self.fallback.and_then(|code| COMMANDS[code][key].as_str()))
            .or_else(|| COMMANDS[Self::ENGLISH.code][key].as_str())
            .unwrap_or("")
    }

    pub(super) fn message(self, key: &str) -> &'static str {
        let mut keys = vec!["guide"];
        keys.extend(key.split('.'));
        self.value(&keys).and_then(Value::as_str).unwrap_or("")
    }

    pub(super) fn lines(self, key: &str) -> Vec<&'static str> {
        let mut keys = vec!["guide"];
        keys.extend(key.split('.'));
        self.value(&keys)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect()
    }

    pub(super) fn name(self, named: Self) -> &'static str {
        self.value(&["languages", named.code])
            .and_then(Value::as_str)
            .unwrap_or(named.code)
    }

    pub(super) fn default_name(self) -> &'static str {
        self.value(&["languages", "default"])
            .and_then(Value::as_str)
            .unwrap_or("Server-Default")
    }

    pub(super) fn progress(self) -> String {
        let pct = self.pack()["progress"].as_f64().unwrap_or_default();
        format!("{pct:?}")
    }

    pub(super) fn flag(self) -> Option<crate::items::HeadTexture> {
        if self.server_default {
            default_flag()
        } else {
            flag(&self.pack()["flag"])
        }
    }

    pub(super) fn category(self, key: &str, fallback: &'static str) -> &'static str {
        let key = match key {
            "usefulItems" => "items",
            "basicMachines" => "basic_machines",
            "magicalResources" => "magical_items",
            "magicalArmor" => "magical_armor",
            "technicalComponents" => "tech_misc",
            "valentinesDay" => "valentines_day",
            "magicalGadgets" => "magical_gadgets",
            "technicalGadgets" => "technical_gadgets",
            other => other,
        };
        self.value(&["categories", key])
            .and_then(Value::as_str)
            .unwrap_or(fallback)
    }

    pub(super) fn item_name(self, id: &str) -> Option<&'static str> {
        if self == Self::CHINESE {
            item_name(id).or_else(|| Self::ENGLISH.item_name(id))
        } else {
            self.value(&["items", id, "name"]).and_then(Value::as_str)
        }
    }

    pub(super) fn recipe_badge(self, id: &str) -> Option<(&'static str, Vec<String>)> {
        if self == Self::CHINESE
            && let Some((name, lore)) = recipe_badge(id)
        {
            return Some((name, lore.into_iter().map(str::to_owned).collect()));
        }
        let name = self.value(&["recipes", id, "name"])?.as_str()?;
        let lore = self
            .value(&["recipes", id, "lore"])?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(|line| format!("&7{line}"))
            .collect();
        Some((name, lore))
    }

    pub(super) fn research_message(self, key: &str) -> &'static str {
        self.value(&["research-messages", key])
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    pub(super) fn research(self, id: &str) -> &'static str {
        if self == Self::CHINESE
            && let Some(name) = ZH_CN["researches"][id].as_str()
        {
            return name;
        }
        self.value(&["researches", id])
            .and_then(Value::as_str)
            .unwrap_or("Unknown research")
    }

    pub(crate) fn localize_components(self, id: &str, components: &mut Compound) {
        if self == Self::CHINESE {
            localize_item_components(id, components);
            return;
        }
        let Some(name) = self.item_name(id) else {
            return;
        };
        components.insert("minecraft:custom_name".into(), colored_lore(name));
        let Some(Tag::List(lore)) = components.get_mut(&"minecraft:lore".into()) else {
            return;
        };
        let Some(lines) = self.pack()["items"][id]["lore"].as_array() else {
            return;
        };
        let replacements: Vec<_> = lines
            .iter()
            .filter_map(|line| {
                let source = colored_lore(line["source"].as_str()?);
                let source = crate::network_runtime::paper_text(&source)?;
                Some((source, line["target"].as_str()?))
            })
            .collect();
        for line in lore {
            if let Some(text) = crate::network_runtime::paper_text(line)
                && let Some((_, target)) = replacements.iter().find(|(source, _)| source == &text)
            {
                *line = colored_lore(target);
            }
        }
    }

    pub(super) fn processing_item(self, host: &dyn mod_api::Host, stack: &mut mod_api::ItemStack) {
        if self == Self::CHINESE {
            return;
        }
        let Some(kind) = crate::items::item_id(stack)
            .and_then(|id| super::catalog::lookup(&id))
            .map(|entry| entry.item.kind)
        else {
            return;
        };
        let Some(default) = kind.stack(host, stack.count()) else {
            return;
        };
        let Some(translated) = kind.guide_stack(host, stack.count(), self) else {
            return;
        };
        let (Some(actual), Some(default), Some(translated)) = (
            stack.components(),
            default.components(),
            translated.components(),
        ) else {
            return;
        };
        let (actual, default, translated) =
            (actual.as_nbt(), default.as_nbt(), translated.as_nbt());
        let mut patch = Compound::new();
        let name = "minecraft:custom_name".into();
        if actual.get(&name) == default.get(&name)
            && let Some(value) = translated.get(&name)
        {
            patch.insert(name, value.clone());
        }
        let lore = "minecraft:lore".into();
        if let (Some(Tag::List(actual)), Some(Tag::List(default)), Some(Tag::List(translated))) =
            (actual.get(&lore), default.get(&lore), translated.get(&lore))
        {
            let mut lines = actual.clone();
            for ((current, source), target) in lines.iter_mut().zip(default).zip(translated) {
                if current == source {
                    *current = target.clone();
                }
            }
            if *actual != lines {
                patch.insert(lore, Tag::List(lines));
            }
        }
        if !patch.is_empty() {
            let _ = stack.apply_components(&patch);
        }
    }
}

fn normalize(value: &str) -> String {
    let lower = value.replace('_', "-").to_ascii_lowercase();
    if matches!(lower.as_str(), "zh" | "zh-cn" | "zh-sg") || lower.starts_with("zh-hans") {
        return "zh-CN".to_owned();
    }
    if matches!(lower.as_str(), "zh-tw" | "zh-hk" | "zh-mo") || lower.starts_with("zh-hant") {
        return "zh-TW".to_owned();
    }
    if let Some(code) = LOCALES["presets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find(|code| code.eq_ignore_ascii_case(&lower))
    {
        return code.to_owned();
    }
    lower.split('-').next().unwrap_or_default().to_owned()
}

pub(super) fn default_flag() -> Option<crate::items::HeadTexture> {
    flag(&LOCALES["default"])
}

fn flag(value: &'static Value) -> Option<crate::items::HeadTexture> {
    let mut profile = [0; 4];
    for (part, value) in profile.iter_mut().zip(value[0].as_array()?) {
        *part = i32::try_from(value.as_i64()?).ok()?;
    }
    Some(crate::items::HeadTexture {
        profile,
        value: value[1].as_str()?,
    })
}

pub(super) fn help_flag() -> Option<crate::items::HeadTexture> {
    flag(&LOCALES["help"])
}

#[derive(Clone, Copy)]
pub(super) struct Settings {
    pub enabled: bool,
    pub default: Language,
    auto_detect: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            default: Language::ENGLISH,
            auto_detect: true,
        }
    }
}

impl Settings {
    pub(super) fn from_config(section: Option<&toml::Value>) -> Self {
        let value = |key| section.and_then(|section| section.get(key));
        Self {
            enabled: value("enable-translations")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true),
            default: value("language")
                .and_then(toml::Value::as_str)
                .and_then(Language::configured)
                .unwrap_or(Language::ENGLISH),
            auto_detect: value("auto-detect-language")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true),
        }
    }

    fn fallback(self) -> Language {
        Language {
            server_default: true,
            ..self.default
        }
    }

    pub(super) fn player(self, host: &dyn mod_api::Host, player: mod_api::PlayerRef) -> Language {
        if !self.enabled {
            return self.fallback();
        }
        let local = player
            .persistent_data(host)
            .and_then(|data| data.get(&PERSISTED.into())?.get(KEY).cloned());
        let selected = local.or_else(|| {
            let key = mod_api::Identifier::parse(KEY).expect("Paper language key");
            player.paper_persistent_value(host, &key)
        });
        resolve(self, selected.as_ref(), || player.client_language(host))
    }
}

fn resolve(
    settings: Settings,
    selected: Option<&Tag>,
    client: impl FnOnce() -> Option<String>,
) -> Language {
    if !settings.enabled {
        return settings.fallback();
    }
    if let Some(Tag::String(selected)) = selected
        && let Some(language) = Language::from_locale(&selected.to_string_lossy())
    {
        return Language {
            fallback: Some(settings.default.code),
            ..language
        };
    }
    if settings.auto_detect
        && let Some(language) = client().as_deref().and_then(Language::from_locale)
    {
        return Language {
            fallback: Some(settings.default.code),
            ..language
        };
    }
    settings.fallback()
}

fn remember(data: &mut Compound, language: Option<Language>) -> bool {
    let Tag::Compound(saved) = data
        .entry(PERSISTED.into())
        .or_insert_with(|| Tag::Compound(Compound::new()))
    else {
        return false;
    };
    // An empty local value clears the choice and keeps a migrated Bukkit preference from resurfacing.
    saved.insert(
        KEY.into(),
        Tag::String(language.map_or("", |language| language.code).into()),
    );
    true
}

pub(super) fn select(
    host: &mut dyn mod_api::ServerHost,
    player: mod_api::PlayerRef,
    language: Option<Language>,
) -> bool {
    let Some(mut data) = player.persistent_data(host) else {
        return false;
    };
    remember(&mut data, language) && player.set_persistent_data(host, data).is_ok()
}

pub(in crate::guide) fn item_name(id: &str) -> Option<&'static str> {
    ZH_CN["items"][id]["name"].as_str()
}

pub(super) fn recipe_badge(id: &str) -> Option<(&'static str, Vec<&'static str>)> {
    let recipe = &ZH_CN["recipes"][id];
    let name = recipe["name"].as_str()?;
    let lore = recipe["lore"]
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<_>>()?;
    Some((name, lore))
}

pub(crate) fn research_name(id: &str) -> &'static str {
    ZH_CN["researches"][id].as_str().unwrap_or("未知研究")
}

fn plain_text(component: &Tag, out: &mut String) {
    match component {
        Tag::String(value) => out.push_str(&value.to_string_lossy()),
        Tag::List(children) => {
            for child in children {
                plain_text(child, out);
            }
        }
        Tag::Compound(_) => {
            if let Some(Tag::String(value)) = component.get("text") {
                out.push_str(&value.to_string_lossy());
            }
            if let Some(Tag::List(children)) = component.get("extra") {
                for child in children {
                    plain_text(child, out);
                }
            }
        }
        _ => {}
    }
}

fn literal(component: &Tag) -> String {
    let mut text = String::new();
    plain_text(component, &mut text);
    text
}

fn translations(id: &str) -> impl Iterator<Item = (&'static str, &'static str)> {
    ZH_CN["items"][id]["lore"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| Some((entry["source"].as_str()?, entry["target"].as_str()?)))
}

fn shared_lore(line: &str) -> Option<&'static str> {
    Some(match line {
        "Cycles through all Colors of the Rainbow!" => "&d在彩虹的各个颜色之间循环切换！",
        "[Christmas Edition]" => "&a[圣诞节版]",
        "[Valentine's Day Edition]" => "&5[&d情人节版&5]",
        "[Halloween Edition]" => "&c[&6万圣节版&c]",
        "Right Click to use" => "&e右键&7 使用",
        "Right Click to open" => "&e右键&7 打开",
        "Crouch to use" => "&e潜行&7 使用",
        _ => return None,
    })
}

/// Localize newly created Slimefun items without changing their identity or state.
pub(crate) fn localize_item_components(id: &str, components: &mut Compound) {
    let name = item_name(id);
    let base = ["ORGANIC_FOOD", "FERTILIZER"].into_iter().find(|base| {
        id.strip_prefix(*base)
            .is_some_and(|suffix| suffix.starts_with('_'))
    });
    if let Some(display_name) = base.and_then(item_name).or(name) {
        components.insert("minecraft:custom_name".into(), colored_lore(display_name));
    }

    let Some(Tag::List(lore)) = components.get_mut(&"minecraft:lore".into()) else {
        return;
    };
    let translations: Vec<_> = translations(id)
        .filter_map(|(source, target)| {
            let styled = crate::network_runtime::paper_text(&colored_lore(source))?;
            Some((styled, target))
        })
        .collect();
    for line in lore {
        let displayed = literal(line);
        let content = base
            .and(name)
            .filter(|_| displayed.starts_with("Content: "));
        let styled = crate::network_runtime::paper_text(line);
        let translated = content
            .or_else(|| {
                let styled = styled.as_deref()?;
                translations
                    .iter()
                    .find_map(|(source, target)| (source == styled).then_some(*target))
            })
            .or_else(|| shared_lore(&displayed));
        if let Some(target) = translated {
            *line = colored_lore(target);
        } else if id == "STAFF_ELEMENTAL_STORM"
            && let Some(target) = styled
                .as_deref()
                .and_then(crate::storm_staff::localize_uses)
        {
            *line = target;
        } else if let Some(target) = styled.as_deref().and_then(crate::machine_lore::localize) {
            *line = colored_lore(&target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picker_only_offers_embedded_paper_presets() {
        let choices: Vec<_> = Language::all().collect();
        assert_eq!(choices.len(), 40);
        assert_eq!(choices[0], Language::ENGLISH);
        assert_eq!(choices[26], Language::CHINESE);
        for unsupported in ["hi", "sr_RS", "si_LK"] {
            assert!(
                Language::from_locale(unsupported).is_none(),
                "{unsupported}"
            );
        }
        assert_eq!(Language::from_locale("pt_BR"), Some(Language::new("pt-BR")));
        assert_eq!(Language::from_locale("zh-Hans-SG"), Some(Language::CHINESE));
        assert_eq!(Language::from_locale("zh_HK"), Some(Language::new("zh-TW")));
        assert_eq!(Language::from_locale("de_DE"), Some(Language::new("de")));
    }

    #[test]
    fn a_saved_choice_beats_client_detection_and_default_can_clear_it() {
        let config = toml::Value::Table(r#"language = "de-DE""#.parse().unwrap());
        let automatic = Settings::from_config(Some(&config));
        let english = || Some("en_us".to_owned());
        let zh_client = || Some("zh_cn".to_owned());
        let unsupported = || Some("qya_aa".to_owned());
        assert_eq!(resolve(automatic, None, zh_client), Language::CHINESE);
        assert!(!resolve(automatic, None, || Some("de_de".to_owned())).is_server_default());
        assert_eq!(resolve(automatic, None, unsupported), Language::new("de"));
        assert!(resolve(automatic, None, unsupported).is_server_default());
        assert_eq!(Language::configured("hi"), Some(Language::new("hi")));
        assert_eq!(Language::configured("si-IN"), Some(Language::new("si")));
        assert_eq!(Language::configured("si-LK"), None);

        let mut data = Compound::from([("unrelated".into(), Tag::Int(73))]);
        assert!(remember(&mut data, Some(Language::CHINESE)));
        let saved = data.get(&PERSISTED.into()).unwrap().get(KEY);
        assert_eq!(resolve(automatic, saved, english), Language::CHINESE);
        assert!(remember(&mut data, None));
        assert_eq!(data.get(&"unrelated".into()), Some(&Tag::Int(73)));
        let cleared = data.get(&PERSISTED.into()).unwrap().get(KEY);
        assert_eq!(resolve(automatic, cleared, english), Language::ENGLISH);
        let forced = Settings {
            auto_detect: false,
            ..automatic
        };
        assert_eq!(resolve(forced, cleared, zh_client), Language::new("de"));
        let disabled = Settings {
            enabled: false,
            ..automatic
        };
        let saved_english = Tag::String("en".into());
        assert_eq!(
            resolve(disabled, Some(&saved_english), zh_client),
            Language::new("de")
        );
    }

    #[test]
    fn selectors_categories_and_stock_item_names_follow_the_active_language() {
        let en = Language::ENGLISH;
        let zh = Language::CHINESE;
        assert_eq!(
            en.message("title.languages"),
            "Select your preferred language"
        );
        assert_eq!(zh.message("title.languages"), "选择你想要更改的语言");
        assert_eq!(en.category("basicMachines", ""), "Basic Machines");
        assert_eq!(zh.category("basicMachines", ""), "基础机器");
        for (language, name) in [
            (en, "Grind Stone"),
            (zh, "磨石"),
            (Language::new("ja"), "Grind Stone"),
        ] {
            assert_eq!(
                literal(&colored_lore(language.item_name("GRIND_STONE").unwrap())),
                name
            );
        }
    }
}
