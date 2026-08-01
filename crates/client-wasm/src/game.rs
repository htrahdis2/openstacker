//! A game in progress, and the recording of it.

use crate::frame::{FRAME_BYTES, Frame};
use engine::{Buttons, Engine, Handling, MatchConfig, TickResult};
use replay::Replay;

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
    seed: u64,
    config: MatchConfig,
    handling: Handling,
}

impl Game {
    pub fn new(seed: u64, config: &MatchConfig, handling: &Handling) -> Game {
        let engine = Engine::new(seed, config, handling);
        let mut game = Game {
            engine,
            frame: [0; FRAME_BYTES],
            last: TickResult::default(),
            inputs: Vec::with_capacity(REPLAY_CAPACITY),
            seed,
            config: config.clone(),
            handling: *handling,
        };
        game.refresh(TickResult::default());
        game
    }

    /// Advance one tick and record the buttons that did it.
    pub fn tick(&mut self, buttons: Buttons) {
        let out = self.engine.tick(buttons);
        self.inputs.push(buttons);
        self.refresh(out);
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
        Replay::record(self.seed, &self.config, &self.handling, &self.inputs)
    }

    fn refresh(&mut self, out: TickResult) {
        self.last = out;
        Frame::read(&self.engine, out).write(&mut self.frame);
    }
}
