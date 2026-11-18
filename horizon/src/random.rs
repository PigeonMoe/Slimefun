// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Mutex;

use mod_api::{Host, LegacyRandom, LevelRef, RandomSource};

/// Slimefun draws use independent state, so they never advance vanilla's level RNG.
#[derive(Default)]
pub(crate) struct Rolls(Mutex<Option<LegacyRandom>>);

impl Rolls {
    pub fn next(&self, host: &dyn Host, level: LevelRef, bound: u32) -> Option<u32> {
        let mut state = self.0.lock().unwrap();
        if state.is_none() {
            *state = Some(level.random(host)?);
        }
        let roll = state
            .as_mut()?
            .next_int_bound(i32::try_from(bound).ok()?)
            .ok()?;
        u32::try_from(roll).ok()
    }

    pub fn clear(&self) {
        *self.0.lock().unwrap() = None;
    }
}
