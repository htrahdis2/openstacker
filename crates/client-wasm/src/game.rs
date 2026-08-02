//! A game in progress, and the recording of it.

use crate::frame::{FRAME_BYTES, Frame};
use crate::sparring::Opponent;
use config::Sparring;
use engine::{Buttons, Engine, Handling, MatchConfig, PendingGarbage, TickResult};
use replay::{Replay, ScheduledGarbage};

/// Ticks reserved for a recording up front: twenty minutes at 60Hz.
///
/// Reserved rather than grown, because growing wasm memory detaches every typed-array
/// view the client built over it.
const REPLAY_CAPACITY: usize = 60 * 60 * 20;

/// One player's game, plus the inputs that produced it.
pub struct Game {
    engine: Engine,
    frame: [u8; FRAME_BYTES],
    last: TickResult,
    inputs: Vec<Buttons>,
    garbage: Vec<ScheduledGarbage>,
    seed: u64,
    config: MatchConfig,
    handling: Handling,
    /// The training opponent, on modes that have one.
    opponent: Option<Opponent>,
}

impl Game {
    pub fn new(seed: u64, config: &MatchConfig, handling: &Handling) -> Game {
        Game::with_opponent(seed, config, handling, None)
    }

    pub fn with_opponent(
        seed: u64,
        config: &MatchConfig,
        handling: &Handling,
        sparring: Option<&Sparring>,
    ) -> Game {
        let engine = Engine::new(seed, config, handling);
        let mut game = Game {
            engine,
            frame: [0; FRAME_BYTES],
            last: TickResult::default(),
            inputs: Vec::with_capacity(REPLAY_CAPACITY),
            garbage: Vec::new(),
            seed,
            config: config.clone(),
            handling: *handling,
            opponent: sparring.map(|p| Opponent::new(seed, p)),
        };
        game.refresh(TickResult::default());
        game
    }

    /// Advance one tick and record the buttons that did it.
    ///
    /// The opponent is asked first, so anything it sends is in the queue for the tick it
    /// was scheduled on rather than the one after — the same ordering `Replay::run` uses
    /// to play the recording back.
    pub fn tick(&mut self, buttons: Buttons) {
        if let Some(mut opponent) = self.opponent.take() {
            let tick = self.inputs.len() as u32 + 1;
            for g in opponent.due(tick, &self.config) {
                self.schedule_garbage(g);
            }
            self.opponent = Some(opponent);
        }
        let out = self.engine.tick(buttons);
        self.inputs.push(buttons);
        self.refresh(out);
    }

    /// Queue rows for a future tick, and record that it happened.
    ///
    /// Recorded against the tick that is about to run, because that is when the engine
    /// receives it: the pending queue is part of the checksum, so a batch replayed into
    /// the queue a tick early or late is a different game.
    pub fn schedule_garbage(&mut self, g: PendingGarbage) {
        self.garbage.push(ScheduledGarbage {
            at_tick: self.inputs.len() as u32 + 1,
            garbage: g,
        });
        self.engine.schedule_garbage(g);
    }

    pub fn frame_bytes(&self) -> &[u8; FRAME_BYTES] {
        &self.frame
    }

    /// The same reading the block holds, typed.
    pub fn frame(&self) -> Frame {
        Frame::read(&self.engine, self.last)
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn checksum(&self) -> u64 {
        self.engine.checksum()
    }

    pub fn is_over(&self) -> bool {
        self.engine.is_over()
    }

    pub fn tick_count(&self) -> usize {
        self.inputs.len()
    }

    /// The recording so far.
    pub fn replay(&self) -> Replay {
        Replay::record(
            self.seed,
            &self.config,
            &self.handling,
            &self.inputs,
            &self.garbage,
        )
    }

    fn refresh(&mut self, out: TickResult) {
        self.last = out;
        Frame::read(&self.engine, out).write(&mut self.frame);
    }
}
