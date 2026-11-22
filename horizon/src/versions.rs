// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Mutex;

use mod_api::{
    DependencyKind, LoadedMods, PackageInfo, RegistrationContext, ServerHost, Text, TextHover,
    text::TextClick,
};

const SLIMEFUN: &str = "slimefun";

#[derive(Default)]
pub(crate) struct Versions(Mutex<Option<LoadedMods>>);

impl Versions {
    pub(crate) fn register(&self, context: &RegistrationContext) {
        *self.0.lock().unwrap() = Some(context.mods().clone());
    }

    pub(crate) fn report(&self, host: &mut dyn ServerHost) {
        let packages = self.0.lock().unwrap().clone();
        let mut text = Text::literal("此服务器使用以下远古工艺配置\n");
        text.style.color = Some("gray".into());
        text.children.push(colored(
            format!(
                "Horizon {} · Minecraft {}\n",
                host.horizon_version(),
                host.game_version()
            ),
            "green",
        ));
        let version = packages
            .as_ref()
            .and_then(|packages| packages.get(SLIMEFUN))
            .map_or(env!("CARGO_PKG_VERSION"), PackageInfo::version);
        text.children
            .push(colored(format!("远古工艺 {version}\n"), "green"));

        let addons: Vec<_> = packages
            .iter()
            .flat_map(LoadedMods::iter)
            .filter(|package| is_addon(package))
            .collect();
        if addons.is_empty() {
            let mut label = colored("没有已加载的远古工艺附属插件。", "gray");
            label.style.italic = Some(true);
            text.children.push(label);
        } else {
            text.children.push(colored(
                format!("已加载的附属插件（{}）", addons.len()),
                "gray",
            ));
            for addon in addons {
                text.children.push(addon_line(addon, host.on(addon.id())));
            }
        }
        let _ = host.commands().feedback(text);
    }
}

fn is_addon(package: &PackageInfo) -> bool {
    package.id() != SLIMEFUN
        && package.dependencies().iter().any(|dependency| {
            dependency.id() == SLIMEFUN
                && matches!(
                    dependency.kind(),
                    DependencyKind::Required | DependencyKind::Optional
                )
        })
}

fn addon_line(package: &PackageInfo, enabled: bool) -> Text {
    let name = if package.name().is_empty() {
        package.id()
    } else {
        package.name()
    };
    let mut line = colored(
        format!("\n  {name} v{}", package.version()),
        if enabled { "green" } else { "red" },
    );
    let mut hover = if enabled {
        if package.authors().is_empty() {
            Text::literal("作者未填写")
        } else {
            Text::literal(format!("作者：{}", package.authors().join("、")))
        }
    } else {
        Text::literal("该附属插件已停用。")
    };
    if let Some(url) = package
        .issues()
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"))
    {
        line.style.click = Some(TextClick::OpenUrl(url.into()));
        hover
            .children
            .push(colored("\n点击打开问题反馈页面", "gold"));
    }
    line.style.hover = Some(TextHover::Text(Box::new(hover)));
    line
}

fn colored(label: impl Into<String>, color: &str) -> Text {
    let mut text = Text::literal(label);
    text.style.color = Some(color.into());
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{DependencyOrder, PackageDependency, PackageDetails, PackageKind, Side};

    fn package(id: &str, kind: DependencyKind) -> PackageInfo {
        let mut details = PackageDetails::default();
        details.name = "测试附属插件".into();
        details.authors = vec!["PigeonMoe".into()];
        details.issues = Some("https://example.invalid/issues".into());
        PackageInfo::new(
            id.into(),
            "1.2.3".into(),
            PackageKind::Code,
            Side::Server,
            vec![id.into()],
            vec![PackageDependency::new(
                SLIMEFUN.into(),
                "*".into(),
                kind,
                DependencyOrder::After,
                Side::Both,
                None,
                None,
            )],
        )
        .with_details(details)
    }

    #[test]
    fn versions_recognize_only_real_addon_dependencies_and_retain_manifest_details() {
        assert!(is_addon(&package("required", DependencyKind::Required)));
        let addon = package("optional", DependencyKind::Optional);
        assert!(is_addon(&addon));
        assert!(!is_addon(&package(
            "incompatible",
            DependencyKind::Incompatible
        )));
        assert!(!is_addon(&package(
            "ordered_first",
            DependencyKind::LoadBefore
        )));
        assert!(!is_addon(&package(SLIMEFUN, DependencyKind::Required)));

        let line = addon_line(&addon, true);
        assert_eq!(
            line.content,
            Text::literal("\n  测试附属插件 v1.2.3").content
        );
        assert_eq!(line.style.color.as_deref(), Some("green"));
        assert_eq!(
            line.style.click,
            Some(TextClick::OpenUrl("https://example.invalid/issues".into()))
        );
        let Some(TextHover::Text(hover)) = line.style.hover else {
            panic!("missing addon authors");
        };
        assert_eq!(hover.content, Text::literal("作者：PigeonMoe").content);
        assert_eq!(
            addon_line(&addon, false).style.color.as_deref(),
            Some("red")
        );
    }
}
