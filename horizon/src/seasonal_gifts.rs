// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, GameMode, Hand, ItemStack, PlayerRef, ServerHost, Tag};

use crate::{
    guide::launch_random_fireworks,
    hologram_projector::colored_lore,
    item_clicks::consume_one,
    items::{self, HeadTexture, Kind, item},
    rainbow::CHRISTMAS_COOKIE,
    random::{Rolls, natural_drop_position},
    recipes::Stack,
};

item!(BIRTHDAY_CAKE, "cake", "Birthday Cake", "aqua", None, None);
item!(
    CHRISTMAS_PRESENT,
    "player_head",
    "Christmas Present",
    "green",
    None,
    Some(HeadTexture {
        profile: [-1107525762, 1623604132, -2034851736, -783029614],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzZjZWY5YWExNGU4ODQ3NzNlYWMxMzRhNGVlODk3MjA2M2Y0NjZkZTY3ODM2M2NmN2IxYTIxYTg1YjcifX19",
    })
);
item!(
    EASTER_EGG,
    "player_head",
    "Easter Egg",
    "white",
    None,
    Some(HeadTexture {
        profile: [-60850122, -247057321, -1115640130, -1113363640],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2IyY2Q1ZGY5ZDdmMWZhODM0MWZjY2UyZjNjMTE4ZTJmNTE3ZTRkMmQ5OWRmMmM1MWQ2MWQ5M2VkN2Y4M2UxMyJ9fX0=",
    })
);

const fn gift(kind: Kind, count: u32) -> Stack {
    Stack { kind, count }
}

const CHRISTMAS: &[Stack] = &[
    gift(items::CHRISTMAS_HOT_CHOCOLATE, 1),
    gift(items::CHRISTMAS_CHOCOLATE_APPLE, 4),
    gift(items::CHRISTMAS_CARAMEL_APPLE, 4),
    gift(items::CHRISTMAS_CAKE, 4),
    gift(CHRISTMAS_COOKIE, 8),
    gift(CHRISTMAS_PRESENT, 1),
    gift(items::CHRISTMAS_EGG_NOG, 1),
    gift(items::CHRISTMAS_MILK, 1),
    gift(items::CHRISTMAS_APPLE_CIDER, 1),
    gift(items::CHRISTMAS_FRUIT_CAKE, 4),
    gift(items::CHRISTMAS_APPLE_PIE, 4),
    gift(Kind::Vanilla("minecraft:emerald"), 1),
];

const EASTER: &[Stack] = &[
    gift(items::CARROT_PIE, 4),
    gift(items::CARROT_JUICE, 1),
    gift(Kind::Vanilla("minecraft:emerald"), 1),
    gift(Kind::Vanilla("minecraft:cake"), 1),
    gift(Kind::Vanilla("minecraft:rabbit_foot"), 1),
    gift(Kind::Vanilla("minecraft:golden_carrot"), 4),
];

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let lore: &[&str] = match id {
        "CHRISTMAS_PRESENT" => {
            components.insert(
                "minecraft:custom_name".into(),
                colored_lore("&aC&ch&ar&ci&as&ct&am&ca&as&c &aP&cr&ae&cs&ae&cn&at"),
            );
            &[
                "&7From: &cTheBusyBiscuit",
                "&7To: &eYou",
                "",
                "&eRight Click &8⇨ &7Open",
            ]
        }
        "EASTER_EGG" => &[
            "&dHappy Easter! Have a surprise.",
            "",
            "&eRight Click &8⇨ &7Open",
        ],
        _ => return,
    };
    components.insert(
        "minecraft:lore".into(),
        Tag::List(lore.iter().copied().map(colored_lore).collect()),
    );
}

pub(crate) fn open(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    held: &ItemStack,
    clicked: Option<([i32; 3], [i32; 3])>,
    rolls: &Rolls,
) -> bool {
    let (gifts, fireworks, position) = if CHRISTMAS_PRESENT.matches(host, held) {
        let Some((block, face)) = clicked else {
            return true;
        };
        let adjacent = std::array::from_fn(|axis| f64::from(block[axis] + face[axis]));
        (CHRISTMAS, 3, Some(adjacent))
    } else if EASTER_EGG.matches(host, held) {
        (EASTER, 2, player.entity().position(host))
    } else {
        return false;
    };

    let Some(mode @ (GameMode::Survival | GameMode::Adventure | GameMode::Creative)) =
        player.game_mode(host)
    else {
        return false;
    };
    let (Some(level), Some(position)) = (player.level(host), position) else {
        return true;
    };
    if mode != GameMode::Creative && !consume_one(host, player, hand, held) {
        return true;
    }
    launch_random_fireworks(host, player, rolls, fireworks);
    if let Some(index) = rolls.next(host, level, gifts.len() as u32)
        && let Some(gift) = gifts[index as usize]
            .kind
            .stack(host, gifts[index as usize].count)
    {
        let [x, y, z] = natural_drop_position(host, level, position);
        let _ = level.spawn_item(host, x, y, z, gift, 10);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_gift_tables_preserve_each_reward_and_its_count() {
        assert_eq!(
            CHRISTMAS
                .iter()
                .map(|gift| (gift.kind.id(), gift.count))
                .collect::<Vec<_>>(),
            [
                ("CHRISTMAS_HOT_CHOCOLATE", 1),
                ("CHRISTMAS_CHOCOLATE_APPLE", 4),
                ("CHRISTMAS_CARAMEL_APPLE", 4),
                ("CHRISTMAS_CAKE", 4),
                ("CHRISTMAS_COOKIE", 8),
                ("CHRISTMAS_PRESENT", 1),
                ("CHRISTMAS_EGG_NOG", 1),
                ("CHRISTMAS_MILK", 1),
                ("CHRISTMAS_APPLE_CIDER", 1),
                ("CHRISTMAS_FRUIT_CAKE", 4),
                ("CHRISTMAS_APPLE_PIE", 4),
                ("minecraft:emerald", 1),
            ]
        );
        assert_eq!(
            EASTER
                .iter()
                .map(|gift| (gift.kind.id(), gift.count))
                .collect::<Vec<_>>(),
            [
                ("CARROT_PIE", 4),
                ("CARROT_JUICE", 1),
                ("minecraft:emerald", 1),
                ("minecraft:cake", 1),
                ("minecraft:rabbit_foot", 1),
                ("minecraft:golden_carrot", 4)
            ]
        );
    }
}
