//! Browser bindings over the simulation.
//!
//! The client crosses this boundary twice per tick: once to advance the game, once to read
//! the frame block. The board is not copied at all — occupancy and colors are read in
//! place, through views built over wasm memory at startup.
//!
//! Strings cross only at construction. Nothing per-tick allocates.

mod frame;
mod game;

pub use frame::{FRAME_BYTES, Frame};
pub use game::Game;

use config::Settings;
use engine::config::desc::Tunable;
use engine::{
    Action, Buttons, ENGINE_VER, Handling, MatchConfig, ms_to_subticks, subticks_to_centiframes,
};
use wasm_bindgen::prelude::*;

/// A game, as the client holds it.
#[wasm_bindgen(js_name = Game)]
pub struct JsGame(Game);

#[wasm_bindgen(js_class = Game)]
impl JsGame {
    /// Start a game from a seed and the rules to play it under.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, config_json: &str, handling_json: &str) -> Result<JsGame, JsError> {
        let config: MatchConfig = serde_json::from_str(config_json)
            .map_err(|e| JsError::new(&format!("match config: {e}")))?;
        let handling: Handling = serde_json::from_str(handling_json)
            .map_err(|e| JsError::new(&format!("handling: {e}")))?;
        Ok(JsGame(Game::new(seed, &config, &handling)))
    }

    /// Advance one tick with the buttons held during it.
    pub fn tick(&mut self, buttons: u8) {
        self.0.tick(Buttons::from_bits_retain(buttons));
    }

    /// Address of the frame block. Stable for the life of the game.
    #[wasm_bindgen(js_name = framePtr)]
    pub fn frame_ptr(&self) -> *const u8 {
        self.0.frame_bytes().as_ptr()
    }

    /// Address of the row bitmasks: one `u16` per row, bit 0 the leftmost column.
    #[wasm_bindgen(js_name = occupancyPtr)]
    pub fn occupancy_ptr(&self) -> *const u16 {
        self.0.engine().board().rows().as_ptr()
    }

    /// Address of the render channel: one byte per cell, indexed `y * 10 + x`.
    #[wasm_bindgen(js_name = colorsPtr)]
    pub fn colors_ptr(&self) -> *const u8 {
        self.0.engine().board().colors().as_ptr()
    }

    pub fn checksum(&self) -> u64 {
        self.0.checksum()
    }

    #[wasm_bindgen(js_name = isOver)]
    pub fn is_over(&self) -> bool {
        self.0.is_over()
    }

    /// The recording so far, as the JSON `replay-cli` reads.
    #[wasm_bindgen(js_name = finishReplay)]
    pub fn finish_replay(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.0.replay())
            .map_err(|e| JsError::new(&format!("could not encode the replay: {e}")))
    }
}

/// Size of the frame block, for building the view over it.
#[wasm_bindgen(js_name = frameBytes)]
pub fn frame_bytes() -> usize {
    FRAME_BYTES
}

/// The button an action resolves to, by the key the settings schema uses.
///
/// Fetched rather than restated, so the client has no second copy of the mapping.
#[wasm_bindgen(js_name = buttonBits)]
pub fn button_bits(action: &str) -> u8 {
    Action::ALL
        .iter()
        .find(|a| a.key() == action)
        .map_or(0, |a| a.button().bits())
}

/// Read stored settings, as `{ settings, notes }`.
///
/// Never fails. `notes` describes anything that had to be adjusted to produce usable
/// settings, in wording meant to be shown to the player.
#[wasm_bindgen(js_name = loadSettings)]
pub fn load_settings(stored: &str) -> String {
    let (settings, notes) = Settings::from_json(stored);
    let notes: Vec<String> = notes.iter().map(ToString::to_string).collect();
    serde_json::json!({ "settings": settings, "notes": notes }).to_string()
}

/// Bring settings into range and up to date, so what is stored is what the engine uses.
#[wasm_bindgen(js_name = normalizeSettings)]
pub fn normalize_settings(json: &str) -> String {
    Settings::from_json(json).0.to_json()
}

#[wasm_bindgen(js_name = defaultSettings)]
pub fn default_settings() -> String {
    Settings::default().to_json()
}

/// A millisecond duration in hundredths of a frame, for read-outs beside a slider.
///
/// Display only: milliseconds are what gets stored, and this quantises.
#[wasm_bindgen]
pub fn centiframes(ms: u32) -> u32 {
    subticks_to_centiframes(ms_to_subticks(ms))
}

/// The rules version. A replay recorded under a different one cannot be re-verified.
#[wasm_bindgen(js_name = engineVer)]
pub fn engine_ver() -> u32 {
    ENGINE_VER
}

/// Default match rules, for a client with no mode selected.
#[wasm_bindgen(js_name = defaultMatchConfig)]
pub fn default_match_config() -> String {
    serde_json::to_string(&MatchConfig::default()).expect("match config should serialize")
}

/// Every handling key that reaches the simulation, in schema order.
#[wasm_bindgen(js_name = handlingKeys)]
pub fn handling_keys() -> String {
    let keys: Vec<&str> = Handling::FIELDS.iter().map(|f| f.key).collect();
    serde_json::to_string(&keys).expect("keys should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Events, Handling, MatchConfig};

    fn game() -> Game {
        Game::new(7, &MatchConfig::default(), &Handling::default())
    }

    /// Drive a game to its end, hard dropping every other tick.
    fn stack_until_over(g: &mut Game, limit: usize) {
        for i in 0..limit {
            let b = if i % 2 == 0 {
                Buttons::HARD_DROP
            } else {
                Buttons::empty()
            };
            g.tick(b);
            if g.is_over() {
                return;
            }
        }
    }

    #[test]
    fn a_new_game_has_a_piece_and_no_ticks() {
        let f = game().frame();
        assert_eq!(f.tick, 0);
        assert_eq!(f.pieces, 1, "the first piece spawns before the first tick");
        assert!(f.active.is_some());
        assert!(f.ghost.is_some());
        assert!(f.hold_kind.is_none());
    }

    #[test]
    fn the_first_tick_is_tick_one() {
        // Garbage arrival is scheduled against absolute ticks, so a client that counts
        // from zero lands rows a frame out.
        let mut g = game();
        g.tick(Buttons::empty());
        assert_eq!(g.frame().tick, 1);
    }

    #[test]
    fn the_preview_is_as_long_as_the_mode_asks_for() {
        let mut config = MatchConfig {
            preview_len: 3,
            ..Default::default()
        };
        config.clamp();
        let g = Game::new(1, &config, &Handling::default());
        let f = g.frame();
        assert_eq!(f.preview_len, 3);
        assert!(f.preview[..3].iter().all(|&k| (1..=7).contains(&k)));
        assert_eq!(f.preview[3..], [0; 4], "unused slots stay empty");
    }

    #[test]
    fn there_is_no_active_piece_during_a_spawn_delay() {
        // The reason the block carries a presence flag: the leftover piece sits exactly on
        // top of the cells it just locked into, so drawing it would duplicate the stack.
        let config = MatchConfig {
            spawn_delay_ms: 100,
            ..Default::default()
        };
        let mut g = Game::new(3, &config, &Handling::default());
        g.tick(Buttons::HARD_DROP);
        let f = g.frame();
        assert_eq!(f.phase, 0, "should be spawning");
        assert!(f.active.is_none());
        assert!(f.ghost.is_none());
    }

    #[test]
    fn a_topped_out_game_reports_it_and_stops_drawing_a_piece() {
        let mut g = game();
        stack_until_over(&mut g, 4000);
        let f = g.frame();
        assert!(f.over, "hard dropping into one column should top out");
        assert!(f.active.is_none());
        assert!(f.ghost.is_none());
    }

    #[test]
    fn holding_fills_the_hold_slot() {
        let mut g = game();
        g.tick(Buttons::HOLD);
        let f = g.frame();
        assert!(f.hold_kind.is_some());
        assert!((1..=7).contains(&f.hold_kind.unwrap()));
    }

    #[test]
    fn the_ghost_sits_below_the_active_piece_in_the_same_columns() {
        let f = game().frame();
        let (active, ghost) = (f.active.unwrap(), f.ghost.unwrap());
        for (a, g) in active.iter().zip(ghost.iter()) {
            assert_eq!(a.0, g.0, "ghost must share the active piece's columns");
            assert!(g.1 >= a.1, "ghost must be at or below the piece");
        }
    }

    #[test]
    fn events_are_reported_for_the_tick_they_happened_on() {
        let mut g = game();
        g.tick(Buttons::HARD_DROP);
        assert!(g.frame().events.contains(Events::PIECE_LOCKED));

        let encoded = u16::from_le_bytes([g.frame_bytes()[20], g.frame_bytes()[21]]);
        assert_eq!(Events::from_bits_retain(encoded), g.frame().events);

        g.tick(Buttons::empty());
        assert!(
            !g.frame().events.contains(Events::PIECE_LOCKED),
            "an event must not survive into the next tick"
        );
    }

    #[test]
    fn the_block_is_written_little_endian_and_fits_its_size() {
        let mut g = game();
        for _ in 0..300 {
            g.tick(Buttons::empty());
        }
        let bytes = g.frame_bytes();
        assert_eq!(bytes.len(), FRAME_BYTES);
        assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 300);
        assert_eq!(&bytes[59..], &[0; 5], "reserved bytes stay zero");
    }

    #[test]
    fn the_recording_reproduces_the_live_game() {
        // The capture path checking itself: if these disagree, the client recorded
        // something other than what it played.
        let mut g = game();
        for i in 0..600 {
            let b = match i % 11 {
                0 => Buttons::LEFT,
                3 => Buttons::CW,
                5 => Buttons::HARD_DROP,
                7 => Buttons::RIGHT,
                9 => Buttons::HOLD,
                _ => Buttons::empty(),
            };
            g.tick(b);
        }
        let replay = g.replay();
        assert_eq!(replay.tick_count(), g.tick_count());
        assert_eq!(replay.claimed.checksum, g.checksum());
        let (_, actual) = replay.simulate();
        assert_eq!(actual, replay.claimed);
    }

    #[test]
    fn every_action_resolves_to_the_button_the_engine_expects() {
        for a in Action::ALL {
            assert_eq!(button_bits(a.key()), a.button().bits(), "{a:?}");
        }
        assert_eq!(button_bits("not_an_action"), 0);
    }

    #[test]
    fn settings_load_with_notes_a_player_can_read() {
        let out = load_settings("{ not json");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let notes = v["notes"].as_array().unwrap();
        assert!(!notes.is_empty(), "an unreadable file must say so");
        assert!(notes[0].as_str().unwrap().len() > 20);
        assert_eq!(v["settings"]["handling"]["das_ms"], 133);
    }

    #[test]
    fn normalizing_settings_brings_them_into_range() {
        let out = normalize_settings(r#"{"handling":{"das_ms":65535}}"#);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["handling"]["das_ms"], 500);
    }

    #[test]
    fn a_frame_readout_matches_the_documented_quantisation() {
        assert_eq!(centiframes(142), 852);
        assert_eq!(centiframes(1000), 6000);
    }

    #[test]
    fn config_json_round_trips_through_the_constructor() {
        let config = default_match_config();
        let parsed: MatchConfig = serde_json::from_str(&config).unwrap();
        assert_eq!(parsed, MatchConfig::default());
    }

    #[test]
    fn handling_keys_match_the_schema() {
        let keys: Vec<String> = serde_json::from_str(&handling_keys()).unwrap();
        assert_eq!(keys.len(), Handling::FIELDS.len());
        assert!(keys.contains(&"das_ms".to_string()));
    }
}
