// SPDX-License-Identifier: GPL-3.0-only

pub(crate) const DEFAULT_SCRIPT: &str = "START-TURN_LEFT-REPEAT";
pub(crate) const MAX_LENGTH: usize = 54;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Normal(Tier),
    Farmer,
    Fisherman(Tier),
    Archived,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tier {
    Solid,
    Liquid,
    Nuclear,
}

impl Tier {
    pub fn number(self) -> u32 {
        match self {
            Self::Solid => 1,
            Self::Liquid => 2,
            Self::Nuclear => 3,
        }
    }
}

impl Mode {
    pub fn tier(self) -> Option<Tier> {
        Some(match self {
            Self::Normal(tier) | Self::Fisherman(tier) => tier,
            Self::Farmer => Tier::Solid,
            Self::Archived => return None,
        })
    }
}

pub(crate) fn mode(id: &str) -> Option<Mode> {
    Some(match id {
        "PROGRAMMABLE_ANDROID" => Mode::Normal(Tier::Solid),
        "PROGRAMMABLE_ANDROID_2" => Mode::Normal(Tier::Liquid),
        "PROGRAMMABLE_ANDROID_3" => Mode::Normal(Tier::Nuclear),
        "PROGRAMMABLE_ANDROID_FARMER" => Mode::Farmer,
        "PROGRAMMABLE_ANDROID_FISHERMAN" => Mode::Fisherman(Tier::Solid),
        "PROGRAMMABLE_ANDROID_2_FISHERMAN" => Mode::Fisherman(Tier::Liquid),
        "PROGRAMMABLE_ANDROID_3_FISHERMAN" => Mode::Fisherman(Tier::Nuclear),
        "PROGRAMMABLE_ANDROID_MINER"
        | "PROGRAMMABLE_ANDROID_WOODCUTTER"
        | "PROGRAMMABLE_ANDROID_BUTCHER"
        | "PROGRAMMABLE_ANDROID_2_FARMER"
        | "PROGRAMMABLE_ANDROID_2_BUTCHER"
        | "PROGRAMMABLE_ANDROID_3_BUTCHER" => Mode::Archived,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Instruction {
    Start,
    Repeat,
    Wait,
    GoForward,
    GoUp,
    GoDown,
    TurnLeft,
    TurnRight,
    CatchFish,
    FarmForward,
    FarmDown,
    InterfaceItems,
    InterfaceFuel,
}

const INSTRUCTIONS: &[(&str, &str, Instruction)] = &[
    ("START", "Start Script", Instruction::Start),
    ("REPEAT", "Repeat Script", Instruction::Repeat),
    ("WAIT", "Wait 0.5s", Instruction::Wait),
    ("GO_FORWARD", "Move forward", Instruction::GoForward),
    ("GO_UP", "Move upwards", Instruction::GoUp),
    ("GO_DOWN", "Move downwards", Instruction::GoDown),
    ("TURN_LEFT", "Turn left", Instruction::TurnLeft),
    ("TURN_RIGHT", "Turn right", Instruction::TurnRight),
    ("CATCH_FISH", "Catch Fish", Instruction::CatchFish),
    (
        "FARM_FORWARD",
        "Harvest and Replant",
        Instruction::FarmForward,
    ),
    (
        "FARM_DOWN",
        "Harvest and Replant (Block underneath)",
        Instruction::FarmDown,
    ),
    (
        "INTERFACE_ITEMS",
        "Push Inventory Contents to the faced Interface",
        Instruction::InterfaceItems,
    ),
    (
        "INTERFACE_FUEL",
        "Pull Fuel from the faced Interface",
        Instruction::InterfaceFuel,
    ),
];

impl Instruction {
    pub fn parse(id: &str) -> Option<Self> {
        INSTRUCTIONS
            .iter()
            .find(|entry| entry.0 == id)
            .map(|entry| entry.2)
    }

    pub fn id(self) -> &'static str {
        INSTRUCTIONS.iter().find(|entry| entry.2 == self).unwrap().0
    }

    pub fn label(self) -> &'static str {
        INSTRUCTIONS.iter().find(|entry| entry.2 == self).unwrap().1
    }

    fn can_use(self, mode: Mode) -> bool {
        match self {
            Self::FarmForward | Self::FarmDown => mode == Mode::Farmer,
            Self::CatchFish => matches!(mode, Mode::Fisherman(_)),
            _ => mode != Mode::Archived,
        }
    }
}

pub(crate) fn choices(mode: Mode) -> impl Iterator<Item = Instruction> {
    INSTRUCTIONS
        .iter()
        .map(|entry| entry.2)
        .filter(move |value| {
            !matches!(value, Instruction::Start | Instruction::Repeat) && value.can_use(mode)
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Script {
    tokens: Vec<String>,
}

impl Default for Script {
    fn default() -> Self {
        Self::parse(DEFAULT_SCRIPT).expect("the stored Paper starting program is valid")
    }
}

impl Script {
    pub fn parse(script: &str) -> Option<Self> {
        let tokens: Vec<_> = script.split('-').map(str::to_owned).collect();
        if !(2..=MAX_LENGTH).contains(&tokens.len())
            || tokens[0] != "START"
            || tokens.last()?.as_str() != "REPEAT"
            || tokens.iter().any(|token| {
                token.is_empty()
                    || token.len() > 64
                    || !token
                        .bytes()
                        .all(|value| value.is_ascii_uppercase() || value == b'_')
            })
        {
            return None;
        }
        Some(Self { tokens })
    }

    pub fn source(&self) -> String {
        self.tokens.join("-")
    }

    pub fn can_run(&self, mode: Mode) -> bool {
        self.tokens.iter().all(|token| {
            Instruction::parse(token).is_some_and(|instruction| instruction.can_use(mode))
        })
    }

    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    pub fn edit(&mut self, index: usize, instruction: Option<Instruction>) -> Option<()> {
        if index == 0 || index >= self.tokens.len() {
            return None;
        }
        if index == self.tokens.len() - 1 {
            if let Some(instruction) = instruction {
                if self.tokens.len() >= MAX_LENGTH {
                    return None;
                }
                self.tokens.insert(index, instruction.id().into());
            }
        } else if let Some(instruction) = instruction {
            self.tokens[index] = instruction.id().into();
        } else {
            self.tokens.remove(index);
        }
        Some(())
    }

    pub fn duplicate(&mut self, index: usize) -> Option<()> {
        if index == 0 || index >= self.tokens.len() - 1 || self.tokens.len() >= MAX_LENGTH {
            return None;
        }
        self.tokens.insert(index + 1, self.tokens[index].clone());
        Some(())
    }

    /// Paper stores the prior index; execution starts at index + 1 and REPEAT returns it to zero.
    pub fn advance(&self, mode: Mode, index: usize) -> Option<(Instruction, usize)> {
        let next = index
            .checked_add(1)
            .filter(|next| *next < self.tokens.len())
            .unwrap_or(0);
        let instruction =
            Instruction::parse(&self.tokens[next]).filter(|part| part.can_use(mode))?;
        let index = if instruction == Instruction::Repeat {
            0
        } else {
            next
        };
        Some((instruction, index))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Facing {
    North,
    East,
    South,
    West,
}

impl Facing {
    const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    pub fn from_id(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|face| face.id().eq_ignore_ascii_case(name))
    }

    pub fn id(self) -> &'static str {
        ["NORTH", "EAST", "SOUTH", "WEST"][self as usize]
    }

    pub fn turned(self, step: i32) -> Self {
        Self::ALL[(self as i32 + step).rem_euclid(4) as usize]
    }

    pub fn offset(self) -> [i32; 3] {
        [[0, 0, -1], [1, 0, 0], [0, 0, 1], [-1, 0, 0]][self as usize]
    }

    // The skull renders facing opposite the script's Paper travel direction.
    pub fn skull_rotation(self) -> u8 {
        [0, 4, 8, 12][self as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_program_waits_in_order_and_repeats_from_start() {
        let mut program = Script::default();
        assert_eq!(
            program.advance(Mode::Normal(Tier::Solid), 0),
            Some((Instruction::TurnLeft, 1))
        );
        assert_eq!(
            program.advance(Mode::Normal(Tier::Solid), 1),
            Some((Instruction::Repeat, 0))
        );
        assert_eq!(program.edit(1, Some(Instruction::FarmDown)), Some(()));
        assert_eq!(program.edit(2, Some(Instruction::Wait)), Some(()));
        assert_eq!(program.duplicate(2), Some(()));
        assert_eq!(program.source(), "START-FARM_DOWN-WAIT-WAIT-REPEAT");
        assert_eq!(program.advance(Mode::Normal(Tier::Solid), 0), None);
        assert_eq!(
            program.advance(Mode::Farmer, 0),
            Some((Instruction::FarmDown, 1))
        );
        assert_eq!(
            program.advance(Mode::Farmer, 1),
            Some((Instruction::Wait, 2))
        );
    }

    #[test]
    fn archived_programs_keep_their_exact_paper_tokens_and_new_tokens() {
        let code = "START-ATTACK_ANIMALS-CATCH_FISH-EXTERNAL_STEP-REPEAT";
        let script = Script::parse(code).unwrap();
        assert_eq!(script.source(), code);
        assert_eq!(script.advance(Mode::Archived, 0), None);
        assert_eq!(script.advance(Mode::Farmer, 0), None);
        assert!(Script::parse("WAIT-FARM_DOWN-REPEAT").is_none());
    }
}
