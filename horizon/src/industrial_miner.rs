// SPDX-License-Identifier: GPL-3.0-only

use std::sync::{Mutex, OnceLock};

use mod_api::{
    BlockState, BlockUpdateFlags, ChunkRef, Host, HostBlockStates, HostTransaction, ITEMS,
    Identifier, ItemStack, LevelRef, ParticleKey, ParticleOptions, PlayerRef, Registrar,
    ServerHost, SoundSource, Text, TicketController, TicketOwner, ValidateTicketsEvent,
};

use crate::{
    fuel_generators::BUCKET_OF_FUEL, heated_pressure::BUCKET_OF_OIL, items::Kind,
    machine::deposit_into, random::Rolls, recipes::Research, research,
};

type Pos = [i32; 3];
const CHEST_SLOTS: usize = 27;
const UP: Pos = [0, 1, 0];
const DOWN: Pos = [0, -1, 0];

static SMOKE: OnceLock<ParticleKey> = OnceLock::new();
static TICKETS: OnceLock<TicketController> = OnceLock::new();

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let _ = SMOKE.set(registrar.particle_key("minecraft:smoke"));
    let _ = TICKETS.set(registrar.register_ticket_controller("industrial_miner", true));
}

pub(crate) fn validate_tickets(event: &mut ValidateTicketsEvent<'_>) {
    if TICKETS.get() == Some(&event.controller) {
        // A Paper mining task lives only until the server stops.
        for (pos, _) in event.validation.block_tickets() {
            event.validation.remove_block_tickets(pos);
        }
    }
}

pub(crate) const RESEARCH: Research = Research {
    id: "industrial_miner",
    name: "Industrial Mining",
    cost: 28,
};
pub(crate) const ADVANCED_RESEARCH: Research = Research {
    id: "advanced_industrial_miner",
    name: "Better Mining",
    cost: 36,
};

#[derive(Clone, Copy, Default)]
struct Rules {
    ancient_debris: bool,
    exclude_deepslate: bool,
}

impl Rules {
    fn from_config(table: &toml::Table, section: &str) -> Self {
        let settings = table.get(section).and_then(toml::Value::as_table);
        let enabled = |name| {
            settings
                .and_then(|table| table.get(name))
                .and_then(toml::Value::as_bool)
        };
        Self {
            ancient_debris: enabled("can-mine-ancient-debris").unwrap_or(false),
            exclude_deepslate: !enabled("can-mine-deepslate-ores").unwrap_or(true),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Piston(usize, bool),
    Fuel,
    Scan,
}

use Stage::{Fuel, Piston, Scan};

// The delays are relative to the previous operation in Paper's TaskQueue.
const WARMUP: &[(i64, Stage)] = &[
    (4, Piston(0, true)),
    (10, Piston(0, false)),
    (8, Piston(1, true)),
    (10, Piston(1, false)),
    (0, Fuel),
    (6, Piston(0, true)),
    (9, Piston(0, false)),
    (4, Piston(1, true)),
    (7, Piston(1, false)),
    (3, Piston(0, true)),
    (5, Piston(0, false)),
    (2, Piston(1, true)),
    (4, Piston(1, false)),
    (1, Piston(0, true)),
    (3, Piston(0, false)),
    (1, Piston(1, true)),
    (2, Piston(1, false)),
];
const CYCLE: &[(i64, Stage)] = &[
    (1, Piston(0, true)),
    (3, Piston(0, false)),
    (1, Piston(1, true)),
    (3, Piston(1, false)),
    (0, Scan),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stopped {
    Finished,
    NoFuel,
    Denied,
    Full,
    Destroyed,
    PistonDirection,
    PistonBlocked,
}

impl Stopped {
    fn notify(self, host: &mut dyn ServerHost, owner: u128, mined: u32) {
        let Some(player) = host.player(owner) else {
            return;
        };
        let message = match self {
            Self::Finished => {
                let _ = player.play_sound(
                    host,
                    "minecraft:entity.arrow.hit_player",
                    SoundSource::Players,
                    0.4,
                    1.0,
                );
                format!(
                    "Your Industrial Miner has finished! It obtained a total of {mined} ore(s)!"
                )
            }
            Self::NoFuel => {
                "Your Industrial Miner ran out of fuel! Put your fuel into the chest above.".into()
            }
            Self::Denied => {
                "You do not seem to have permission to operate an Industrial Miner here!".into()
            }
            Self::Full => "The Chest of your Industrial Miner is full!".into(),
            Self::Destroyed => "Your Industrial Miner seems to have been destroyed.".into(),
            Self::PistonDirection => {
                "Your Industrial Miner requires pistons to face upwards!".into()
            }
            Self::PistonBlocked => "The two pistons need to have an empty block above them!".into(),
        };
        let _ = player.tell(host, Text::literal(message));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Columns {
    x: i32,
    z: i32,
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
}

impl Columns {
    fn square([x, _, z]: Pos, radius: i32) -> Self {
        Self {
            x: x - radius,
            z: z - radius,
            min_x: x - radius,
            max_x: x + radius,
            min_z: z - radius,
            max_z: z + radius,
        }
    }

    fn chunks(&self) -> impl Iterator<Item = [i32; 2]> + '_ {
        (self.min_x.div_euclid(16)..=self.max_x.div_euclid(16)).flat_map(|x| {
            (self.min_z.div_euclid(16)..=self.max_z.div_euclid(16)).map(move |z| [x, z])
        })
    }

    fn advance(&mut self) -> bool {
        if self.x < self.max_x {
            self.x += 1;
        } else if self.z < self.max_z {
            self.x = self.min_x;
            self.z += 1;
        } else {
            return false;
        }
        true
    }
}

struct Job {
    level: LevelRef,
    origin: Pos,
    pistons: [Pos; 2],
    owner: u128,
    advanced: bool,
    columns: Columns,
    chunks: Vec<ChunkRef>,
    fuel: u32,
    mined: u32,
    tasks: &'static [(i64, Stage)],
    next: i64,
}

#[derive(Default)]
pub(crate) struct Miners {
    running: Mutex<Vec<Job>>,
    rules: Mutex<[Rules; 2]>,
}

impl Miners {
    pub(crate) fn configure(&self, config: Option<&toml::Table>) {
        *self.rules.lock().unwrap() = config.map_or([Rules::default(); 2], |config| {
            [
                Rules::from_config(config, "industrial-miner"),
                Rules::from_config(config, "advanced-industrial-miner"),
            ]
        });
    }

    pub(crate) fn activate(
        &self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        player: PlayerRef,
        pos: Pos,
    ) -> bool {
        let Some((advanced, pistons)) = structure(host, level, pos) else {
            return false;
        };
        let id = if advanced { "ADVANCED_INDUSTRIAL_MINER" } else { "INDUSTRIAL_MINER" };
        if let Some(denied) = crate::availability::use_denial(host.dimension(), id) {
            denied.tell(host, player);
            return true;
        }
        let required = if advanced {
            &ADVANCED_RESEARCH
        } else {
            &RESEARCH
        };
        if !research::can_use(host, player, required) {
            let _ = player.tell(
                host,
                Text::literal(format!(
                    "Unlock this research with /sf research {}.",
                    required.id
                )),
            );
            return true;
        }
        let Some(now) = level.game_time(host) else {
            return true;
        };
        let mut jobs = self.running.lock().unwrap();
        if jobs
            .iter()
            .any(|job| job.level == level && job.origin == pos)
        {
            drop(jobs);
            let _ = player.tell(
                host,
                Text::literal("This Industrial Miner is already running!"),
            );
            return true;
        }
        let columns = Columns::square(pos, if advanced { 5 } else { 3 });
        let Some(chunks) = columns
            .chunks()
            .map(|pos| level.chunk_ref(host, pos))
            .collect()
        else {
            Stopped::Destroyed.notify(host, player.uuid(), 0);
            return true;
        };
        let job = Job {
            level,
            origin: pos,
            pistons,
            owner: player.uuid(),
            advanced,
            columns,
            chunks,
            fuel: 0,
            mined: 0,
            tasks: WARMUP,
            next: now.saturating_add(WARMUP[0].0),
        };
        if job.hold_chunks(host) {
            jobs.push(job);
        } else {
            Stopped::Destroyed.notify(host, player.uuid(), 0);
        }
        true
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef, rolls: &Rolls) {
        let Some(now) = level.game_time(host) else {
            return;
        };
        let jobs = self
            .running
            .lock()
            .unwrap()
            .extract_if(.., |job| job.level == level && job.next <= now)
            .collect::<Vec<_>>();
        let rules = *self.rules.lock().unwrap();
        for mut job in jobs {
            match job.step(host, rolls, rules[usize::from(job.advanced)], now) {
                Ok(()) => self.running.lock().unwrap().push(job),
                Err(reason) => {
                    job.release_chunks(host);
                    reason.notify(host, job.owner, job.mined);
                }
            }
        }
    }

    pub(crate) fn unload(&self, host: &mut dyn ServerHost, dimension: &str) {
        let jobs = self
            .running
            .lock()
            .unwrap()
            .extract_if(.., |job| {
                job.level.dimension().id(host).as_deref() == Some(dimension)
            })
            .collect::<Vec<_>>();
        for job in jobs {
            job.release_chunks(host);
        }
    }

    pub(crate) fn clear(&self, host: &mut dyn ServerHost) {
        for job in std::mem::take(&mut *self.running.lock().unwrap()) {
            job.release_chunks(host);
        }
    }
}

impl Job {
    fn hold_chunks(&self, host: &mut dyn ServerHost) -> bool {
        let Some(&controller) = TICKETS.get() else {
            return false;
        };
        let owner = TicketOwner::Block(self.origin);
        let failed_at = self
            .chunks
            .iter()
            .position(|chunk| !chunk.set_mod_forced(host, controller, owner, true, false));
        if let Some(failed_at) = failed_at {
            for chunk in &self.chunks[..failed_at] {
                chunk.set_mod_forced(host, controller, owner, false, false);
            }
        }
        failed_at.is_none()
    }

    fn release_chunks(&self, host: &mut dyn ServerHost) {
        let Some(&controller) = TICKETS.get() else {
            return;
        };
        for chunk in &self.chunks {
            chunk.set_mod_forced(
                host,
                controller,
                TicketOwner::Block(self.origin),
                false,
                false,
            );
        }
    }

    fn step(
        &mut self,
        host: &mut dyn ServerHost,
        rolls: &Rolls,
        rules: Rules,
        now: i64,
    ) -> Result<(), Stopped> {
        if self.chunks.iter().any(|chunk| !chunk.loaded(host)) {
            self.next = now.saturating_add(1);
            return Ok(());
        }
        while self.next <= now {
            let (_, stage) = self.tasks[0];
            self.tasks = &self.tasks[1..];
            let delay = match stage {
                Piston(slot, extended) => {
                    self.animate(host, self.pistons[slot], extended)?;
                    if self.tasks.is_empty() { 1 } else { 0 }
                }
                Fuel => {
                    self.refuel(host)?;
                    0
                }
                Scan => {
                    if self.scan(host, rolls, rules)? {
                        4
                    } else if self.columns.advance() {
                        5
                    } else {
                        return Err(Stopped::Finished);
                    }
                }
            };
            if self.tasks.is_empty() {
                self.tasks = CYCLE;
            }
            self.next = now.saturating_add(delay + self.tasks[0].0);
        }
        Ok(())
    }

    fn animate(
        &self,
        host: &mut dyn ServerHost,
        piston: Pos,
        extended: bool,
    ) -> Result<(), Stopped> {
        if let Some(&smoke) = SMOKE.get() {
            self.level.send_particles(
                host,
                &ParticleOptions::simple(smoke),
                None,
                self.origin.map(f64::from),
                [0.7; 3],
                0.0,
                20,
            );
        }
        let above = add(piston, UP);
        let state = self
            .level
            .block_state(host, piston)
            .ok_or(Stopped::Destroyed)?;
        let name = state.block().id(host);
        if name == "minecraft:moving_piston" {
            return clear_piston_head(host, self.level, above);
        }
        if name != "minecraft:piston" {
            return Err(Stopped::Destroyed);
        }
        let top = self
            .level
            .block_state(host, above)
            .ok_or(Stopped::PistonBlocked)?;
        if !is_air(&top.block().id(host)) && top.block().id(host) != "minecraft:piston_head" {
            return Err(Stopped::PistonBlocked);
        }
        let metadata = host.block_states();
        if state
            .property(&metadata, "facing")
            .ok()
            .flatten()
            .as_deref()
            != Some("up")
        {
            return Err(Stopped::PistonDirection);
        }
        let next = state
            .with(
                &metadata,
                "extended",
                if extended { "true" } else { "false" },
            )
            .map_err(|_| Stopped::Destroyed)?;
        let flags = BlockUpdateFlags::CLIENTS | BlockUpdateFlags::KNOWN_SHAPE;
        self.level
            .set_block(host, piston, next, flags)
            .map_err(|_| Stopped::Destroyed)?;
        if extended {
            let head = default_state(host, "minecraft:piston_head")
                .ok_or(Stopped::Destroyed)?
                .with(&host.block_states(), "facing", "up")
                .map_err(|_| Stopped::Destroyed)?;
            self.level
                .set_block(host, above, head, flags)
                .map_err(|_| Stopped::PistonBlocked)?;
        } else {
            clear_piston_head(host, self.level, above)?;
        }
        let sound = if extended {
            "minecraft:block.piston.extend"
        } else {
            "minecraft:block.piston.contract"
        };
        let _ = self.level.play_sound(
            host,
            sound,
            piston.map(f64::from),
            SoundSource::Blocks,
            0.1,
            1.0,
        );
        Ok(())
    }

    fn refuel(&mut self, host: &mut dyn ServerHost) -> Result<(), Stopped> {
        let chest = add(self.origin, UP);
        if !is_block(host, self.level, chest, "minecraft:chest") {
            return Err(Stopped::Destroyed);
        }
        let handler = self
            .level
            .capability(host, &ITEMS, chest, None)
            .ok_or(Stopped::Destroyed)?;
        let (slot, resource, fuel) = (0..CHEST_SLOTS.min(handler.size(host)))
            .find_map(|slot| {
                let resource = handler.resource(host, slot)?;
                let stack = resource.to_stack(1)?;
                fuel_value(host, &stack, self.advanced).map(|fuel| (slot, resource, fuel))
            })
            .ok_or(Stopped::NoFuel)?;
        host.transaction(|host, tx| {
            if handler.extract(host, tx, slot, &resource, 1) != 1 {
                return Err(Stopped::NoFuel);
            }
            if self.advanced
                && let Some(bucket) = Kind::Vanilla("minecraft:bucket").stack(host, 1)
            {
                let _ = deposit_into(host, tx, self.level, chest, CHEST_SLOTS, &bucket, 1);
            }
            Ok(())
        })?;
        self.fuel = fuel;
        Ok(())
    }

    fn scan(
        &mut self,
        host: &mut dyn ServerHost,
        rolls: &Rolls,
        rules: Rules,
    ) -> Result<bool, Stopped> {
        if let Some(stone) = default_state(host, "minecraft:stone") {
            let _ = self.level.block_break_effect(host, self.origin, stone);
        }
        let (min_y, _) = self.level.height(host).ok_or(Stopped::Destroyed)?;
        for y in (min_y + 1..self.origin[1]).rev() {
            let pos = [self.columns.x, y, self.columns.z];
            if !self.level.allow_device_breaking(host, self.owner, pos) {
                return Err(Stopped::Denied);
            }
            let state = self.level.block_state(host, pos).ok_or(Stopped::Denied)?;
            let name = state.block().id(host);
            let Some((material, base, range)) = ore(&name, self.advanced, rules) else {
                continue;
            };
            let count = base
                + if range > 1 {
                    rolls
                        .next(host, self.level, range)
                        .ok_or(Stopped::Destroyed)?
                } else {
                    0
                };
            if crate::network_runtime::has_block_marker(host, self.level, pos) {
                continue;
            }
            let id = Identifier::parse(material).map_err(|_| Stopped::Destroyed)?;
            let key = host.item_key(&id).ok_or(Stopped::Destroyed)?;
            let product = ItemStack::new(key, count).ok_or(Stopped::Destroyed)?;
            if self.fuel == 0 {
                self.refuel(host)?;
            }
            let chest = add(self.origin, UP);
            let air = default_state(host, "minecraft:air").ok_or(Stopped::Destroyed)?;
            if !is_block(host, self.level, chest, "minecraft:chest") {
                return Err(Stopped::Destroyed);
            }
            host.transaction(|host, tx| {
                if deposit_into(host, tx, self.level, chest, CHEST_SLOTS, &product, count) != 0 {
                    return Err(Stopped::Full);
                }
                if self.level.block_state(host, pos) != Some(state)
                    || self.level.set_block(
                        host,
                        pos,
                        air,
                        BlockUpdateFlags::CLIENTS | BlockUpdateFlags::NEIGHBORS,
                    ) != Ok(true)
                {
                    return Err(Stopped::Destroyed);
                }
                Ok(())
            })?;
            let _ = self.level.block_break_effect(host, self.origin, state);
            let _ = self.level.play_sound(
                host,
                "minecraft:entity.arrow.hit_player",
                self.origin.map(f64::from),
                SoundSource::Blocks,
                0.2,
                1.0,
            );
            self.fuel -= 1;
            self.mined += 1;
            return Ok(true);
        }
        Ok(false)
    }
}

fn structure(host: &dyn Host, level: LevelRef, origin: Pos) -> Option<(bool, [Pos; 2])> {
    let block = |pos, id| is_block(host, level, pos, id);
    let chest = add(origin, UP);
    if !block(origin, "minecraft:blast_furnace") || !block(chest, "minecraft:chest") {
        return None;
    }
    for axis in [[0, 0, -1], [-1, 0, 0]] {
        let sides = [axis, axis.map(|c| -c)].map(|delta| add(chest, delta));
        if sides.iter().any(|&side| !block(side, "minecraft:piston")) {
            continue;
        }
        for (advanced, material) in [
            (false, "minecraft:iron_block"),
            (true, "minecraft:diamond_block"),
        ] {
            if sides.iter().all(|&side| block(add(side, DOWN), material)) {
                return Some((advanced, sides));
            }
        }
    }
    None
}

pub(crate) fn fuel_value(host: &dyn Host, stack: &ItemStack, advanced: bool) -> Option<u32> {
    let name = stack.item().id(host);
    let seconds = if advanced {
        [
            (Kind::Vanilla("minecraft:lava_bucket"), 48),
            (BUCKET_OF_OIL, 64),
            (BUCKET_OF_FUEL, 128),
        ]
        .into_iter()
        .find_map(|(kind, value)| kind.matches(host, stack).then_some(value))
    } else {
        match name.as_str() {
            "minecraft:coal" | "minecraft:charcoal" | "minecraft:blaze_rod" => Some(4),
            "minecraft:coal_block" => Some(40),
            "minecraft:dried_kelp_block" => Some(10),
            _ => host
                .item_block(stack.item())
                .filter(|&block| host.block_in_tag(block, "minecraft:logs") == Some(true))
                .map(|_| 1),
        }
    }?;
    // Paper's MachineFuel converts seconds to ticks. The miner spends one tick per ore.
    Some(seconds * 2)
}

fn ore(name: &str, advanced: bool, rules: Rules) -> Option<(&str, u32, u32)> {
    let local = name.strip_prefix("minecraft:")?;
    let base = match local.strip_prefix("deepslate_") {
        Some(
            ore @ ("coal_ore" | "diamond_ore" | "emerald_ore" | "redstone_ore" | "lapis_ore"
            | "copper_ore" | "iron_ore" | "gold_ore"),
        ) if !rules.exclude_deepslate => ore,
        Some(_) => return None,
        None => local,
    };
    let normal = match base {
        "coal_ore" => ("minecraft:coal", 1, 1),
        "diamond_ore" => ("minecraft:diamond", 1, 1),
        "emerald_ore" => ("minecraft:emerald", 1, 1),
        "redstone_ore" => ("minecraft:redstone", 4, 2),
        "lapis_ore" => ("minecraft:lapis_lazuli", 4, 4),
        "copper_ore" => ("minecraft:raw_copper", 1, 1),
        "iron_ore" => ("minecraft:raw_iron", 1, 1),
        "gold_ore" => ("minecraft:raw_gold", 1, 1),
        "nether_quartz_ore" => ("minecraft:quartz", 1, 1),
        "nether_gold_ore" => ("minecraft:gold_nugget", 2, 4),
        "gilded_blackstone" => (name, 1, 1),
        "ancient_debris" if rules.ancient_debris => (name, 1, 1),
        _ => return None,
    };
    Some(if advanced { (name, 1, 1) } else { normal })
}

fn add(a: Pos, b: Pos) -> Pos {
    std::array::from_fn(|i| a[i] + b[i])
}
fn is_air(id: &str) -> bool {
    matches!(
        id,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}
fn is_block(host: &dyn Host, level: LevelRef, pos: Pos, id: &str) -> bool {
    level
        .block(host, pos)
        .is_some_and(|block| block.id(host) == id)
}
fn default_state(host: &dyn Host, id: &str) -> Option<BlockState> {
    host.block_state_default(host.block_key(id)?)
}
fn clear_piston_head(host: &mut dyn ServerHost, level: LevelRef, pos: Pos) -> Result<(), Stopped> {
    let air = default_state(host, "minecraft:air").ok_or(Stopped::Destroyed)?;
    level
        .set_block(
            host,
            pos,
            air,
            BlockUpdateFlags::CLIENTS | BlockUpdateFlags::NEIGHBORS,
        )
        .map_err(|_| Stopped::Destroyed)?;
    Ok(())
}

pub(crate) fn recipe_description(id: &str) -> Option<String> {
    let (name, base, fuel, area) = match id.to_ascii_uppercase().as_str() {
        "INDUSTRIAL_MINER" => ("Industrial Miner", "iron", "coal or similar fuel", "7×7"),
        "ADVANCED_INDUSTRIAL_MINER" => (
            "Advanced Industrial Miner",
            "diamond",
            "lava, oil or fuel buckets",
            "11×11 with Silk Touch",
        ),
        _ => return None,
    };
    Some(format!(
        "{name} | Set a chest on a blast furnace and flank the chest with upward-facing pistons on {base} blocks. Place {fuel} in the chest and use the furnace. Mines {area}."
    ))
}

pub(crate) fn guide_lore(id: &str) -> Option<&'static [&'static str]> {
    match id {
        "INDUSTRIAL_MINER" => Some(&[
            "",
            "&fThis Multiblock will mine any Ores",
            "&fin a 7x7 area underneath it.",
            "&fPlace coal or similar in its chest",
            "&fto fuel this machine.",
        ]),
        "ADVANCED_INDUSTRIAL_MINER" => Some(&[
            "",
            "&fThis Multiblock will mine any Ores",
            "&fin a 11x11 area underneath it.",
            "&fPlace a bucket of fuel or lava in",
            "&fits chest to fuel this machine.",
            "",
            "&a+ Silk Touch",
        ]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_paper_scan_order_from_low_x_and_low_z() {
        let mut columns = Columns::square([7, 64, -4], 3);
        let mut visited = vec![[columns.x, columns.z]];
        while columns.advance() {
            visited.push([columns.x, columns.z]);
        }
        assert_eq!(visited.len(), 49);
        assert_eq!(
            &visited[0..8],
            &[
                [4, -7],
                [5, -7],
                [6, -7],
                [7, -7],
                [8, -7],
                [9, -7],
                [10, -7],
                [4, -6]
            ]
        );
        assert_eq!(visited[48], [10, -1]);
    }

    #[test]
    fn tickets_cover_every_chunk_once_when_the_miner_spans_boundaries() {
        let columns = Columns::square([0, 63, -16], 5);
        assert_eq!(
            columns.chunks().collect::<Vec<_>>(),
            [[-1, -2], [-1, -1], [0, -2], [0, -1]]
        );
        let columns = Columns::square([8, 63, -8], 5);
        assert_eq!(columns.chunks().collect::<Vec<_>>(), [[0, -1]]);
    }

    #[test]
    fn a_new_fuel_item_grants_one_paper_mining_use_per_fuel_tick() {
        use mod_api::{Identifier, testing::FakeHost};

        mod_api::__private::initialize().unwrap();
        let mut host = FakeHost::new();
        let fuels = [
            (Kind::Vanilla("minecraft:coal"), false, 8),
            (Kind::Vanilla("minecraft:coal_block"), false, 80),
            (Kind::Vanilla("minecraft:lava_bucket"), true, 96),
            (BUCKET_OF_OIL, true, 128),
            (BUCKET_OF_FUEL, true, 256),
        ];
        host.registries.resolve_items(fuels.map(|(kind, _, _)| {
            Identifier::parse(kind.material()).unwrap()
        })).unwrap();
        for (kind, advanced, ores) in fuels {
            let stack = kind.stack(&host, 1).unwrap();
            assert_eq!(fuel_value(&host, &stack, advanced), Some(ores));
        }
    }

    #[test]
    fn ore_dictionary_preserves_raw_drop_ranges_silk_touch_and_settings() {
        let mut rules = Rules::default();
        assert_eq!(
            ore("minecraft:deepslate_lapis_ore", false, rules),
            Some(("minecraft:lapis_lazuli", 4, 4))
        );
        assert_eq!(
            ore("minecraft:deepslate_iron_ore", true, rules),
            Some(("minecraft:deepslate_iron_ore", 1, 1))
        );
        assert_eq!(
            ore("minecraft:nether_gold_ore", false, rules),
            Some(("minecraft:gold_nugget", 2, 4))
        );
        assert_eq!(ore("minecraft:ancient_debris", true, rules), None);
        rules.ancient_debris = true;
        rules.exclude_deepslate = true;
        assert_eq!(
            ore("minecraft:ancient_debris", true, rules),
            Some(("minecraft:ancient_debris", 1, 1))
        );
        assert_eq!(ore("minecraft:deepslate_diamond_ore", true, rules), None);
        assert_eq!(
            ore("minecraft:gilded_blackstone", false, rules),
            Some(("minecraft:gilded_blackstone", 1, 1))
        );
    }
}
