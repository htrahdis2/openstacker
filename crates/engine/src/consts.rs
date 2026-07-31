//! Compile-time constants.
//!
//! `BOARD_W`, `BOARD_H` and `TICK_HZ` are deliberately not tunable. Board width is baked
//! into the `u16` row bitmasks that every collision, clear and checksum routine derives
//! from, so making it dynamic would infect the whole engine for no real benefit.

/// Playfield width in columns. Also the bit-width of a row mask.
pub const BOARD_W: usize = 10;

/// Total playfield height: 20 visible rows plus 20 rows of spawn buffer.
///
/// Row `0` is the **top** of the buffer; row `BOARD_H - 1` is the floor. Gravity
/// increases `y`.
pub const BOARD_H: usize = 40;

/// Rows `VISIBLE_H..BOARD_H` are shown to the player. Rows `0..VISIBLE_H` are buffer.
pub const VISIBLE_H: usize = 20;

/// Simulation rate. One `Engine::tick` is one of these.
pub const TICK_HZ: u32 = 60;

/// A row mask with every column occupied.
pub const FULL_ROW: u16 = 0b11_1111_1111;

/// Upper bound on `MatchConfig::preview_len`, sizing the preview accessor.
pub const MAX_PREVIEW: usize = 7;

/// Subdivisions of one tick. Every timer in the engine counts these, which is what
/// buys sub-frame handling precision without floats.
pub const SUBTICK: u32 = 256;

/// Bumped on **any** change to simulation-affecting rules: kick tables, handling
/// semantics, scoring, RNG, spawn positions, garbage application, or the checksum byte
/// order.
///
/// A bump invalidates stored replays for *verification*. They stay playable and are
/// marked unverifiable rather than being silently re-verified under the new rules.
pub const ENGINE_VER: u32 = 1;

/// Color index reserved for empty cells in the render channel.
pub const COLOR_EMPTY: u8 = 0;

/// Color index reserved for garbage rows in the render channel.
pub const COLOR_GARBAGE: u8 = 8;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_row_covers_exactly_board_w_columns() {
        assert_eq!(FULL_ROW.count_ones() as usize, BOARD_W);
        assert_eq!(FULL_ROW, (1u16 << BOARD_W) - 1);
    }

    #[test]
    fn buffer_is_as_tall_as_the_visible_field() {
        assert_eq!(BOARD_H - VISIBLE_H, VISIBLE_H);
    }

    #[test]
    fn subtick_is_a_power_of_two() {
        // Not load-bearing for correctness, but it keeps the ms conversion exact
        // and lets tick counts be recovered with a shift.
        assert!(SUBTICK.is_power_of_two());
    }
}
