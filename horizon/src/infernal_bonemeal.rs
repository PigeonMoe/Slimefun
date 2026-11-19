// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    BlockState, BlockUpdateFlags, Flow, GameMode, HostBlockStates, HostTransaction,
    InteractionResult, ItemPlace, ItemResource, PlayerRef, RightClickBlockEvent, ServerHost, Tag,
    Text, TriState,
};

use crate::{
    ancient_altar::ANCIENT_RUNE_EARTH,
    items::{Kind, MAGIC_LUMP_2, item, text},
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    INFERNAL_BONEMEAL,
    "bone_meal",
    "Infernal Bonemeal",
    "dark_red",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "infernal_bonemeal",
    name: "Infernal Bonemeal",
    cost: 12,
};

const WART: Kind = Kind::Vanilla("minecraft:nether_wart");

pub(crate) const ALTAR: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(WART),
        Some(ANCIENT_RUNE_EARTH),
        Some(WART),
        Some(MAGIC_LUMP_2),
        Some(Kind::Vanilla("minecraft:bone_meal")),
        Some(MAGIC_LUMP_2),
        Some(WART),
        Some(Kind::Vanilla("minecraft:blaze_powder")),
        Some(WART),
    ],
    output: Stack {
        kind: INFERNAL_BONEMEAL,
        count: 8,
    },
    research: &RESEARCH,
};

pub(crate) fn lore() -> Tag {
    Tag::List(vec![
        Tag::String("".into()),
        text("Speeds up the Growth of", "red", false),
        text("Nether Warts as well", "red", false),
    ])
}

fn mature(host: &dyn ServerHost, state: BlockState) -> Option<BlockState> {
    if state.block().id(host) != WART.material() {
        return None;
    }
    let metadata = host.block_states();
    if state.property(&metadata, "age").ok()?? == "3" {
        return None;
    }
    state.with(&metadata, "age", "3").ok()
}

pub(crate) fn use_on(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    event: &mut RightClickBlockEvent<'_>,
) -> Flow {
    let Some(stack) = event
        .click
        .stack
        .filter(|stack| INFERNAL_BONEMEAL.matches(host, stack))
    else {
        return Flow::Continue;
    };
    if *event.use_item == TriState::False {
        return Flow::Continue;
    }
    if !research::can_use(host, player, &RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock Infernal Bonemeal with /sf research infernal_bonemeal."),
        );
        *event.use_item = TriState::False;
        return Flow::Continue;
    }
    *event.use_block = TriState::False;

    let Some(level) = player.level(host) else {
        return Flow::Continue;
    };
    let pos = event.click.pos;
    let Some(old) = level.block_state(host, pos) else {
        return Flow::Continue;
    };
    let Some(grown) = mature(host, old) else {
        // For other plants the unchanged held stack follows vanilla's bone meal path.
        return Flow::Continue;
    };
    let hand = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: event.click.hand,
    };
    let source = ItemResource::of(stack);
    let creative = player.game_mode(host) == Some(GameMode::Creative);
    let changed = host.transaction(|host, tx| {
        if level.block_state(host, pos) != Some(old)
            || (!creative && hand.extract(host, tx, &source, 1) != 1)
            || !level
                .set_block(
                    host,
                    pos,
                    grown,
                    BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                )
                .map_err(|_| ())?
        {
            return Err(());
        }
        Ok(())
    });
    if changed.is_err() {
        return Flow::Continue;
    }
    if let Some(redstone) = host.block_state_id("minecraft:redstone_block") {
        level.block_break_effect(host, pos, redstone);
    }
    *event.cancellation_result = InteractionResult::Consume;
    Flow::Cancel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_preserves_paper_catalyst_ring_and_batch_size() {
        assert_eq!(
            ALTAR.inputs.map(|entry| entry.map(Kind::id)),
            [
                Some("minecraft:nether_wart"),
                Some("ANCIENT_RUNE_EARTH"),
                Some("minecraft:nether_wart"),
                Some("MAGIC_LUMP_2"),
                Some("minecraft:bone_meal"),
                Some("MAGIC_LUMP_2"),
                Some("minecraft:nether_wart"),
                Some("minecraft:blaze_powder"),
                Some("minecraft:nether_wart")
            ],
        );
        assert_eq!(
            (
                ALTAR.output.kind.id(),
                ALTAR.output.count,
                ALTAR.research.cost
            ),
            ("INFERNAL_BONEMEAL", 8, 12)
        );
    }
}
