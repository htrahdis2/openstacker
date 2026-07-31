//! The playfield bitboard.
//!
//! Occupancy is one `u16` per row, one bit per column, bit `0` = column `0` (leftmost).
//! Row `0` is the top of the spawn buffer and row `BOARD_H - 1` is the floor, so
//! gravity increases `y`.
//!
//! `colors` is a strict **render channel**. Game logic never reads it, and the state
//! checksum excludes it entirely, so that a cosmetic change can never cause a desync.
//! It lives here rather than alongside the board so that row collapse stays atomic
//! across both representations.

use crate::consts::{BOARD_H, BOARD_W, COLOR_EMPTY, COLOR_GARBAGE, FULL_ROW};
use core::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct Board {
    rows: [u16; BOARD_H],
    colors: [u8; BOARD_W * BOARD_H],
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    pub const fn new() -> Self {
        Board {
            rows: [0; BOARD_H],
            colors: [COLOR_EMPTY; BOARD_W * BOARD_H],
        }
    }

    // ---- reads -------------------------------------------------------------

    #[inline]
    pub const fn row(&self, y: usize) -> u16 {
        self.rows[y]
    }

    #[inline]
    pub const fn rows(&self) -> &[u16; BOARD_H] {
        &self.rows
    }

    #[inline]
    pub const fn colors(&self) -> &[u8; BOARD_W * BOARD_H] {
        &self.colors
    }

    /// Whether a cell inside the playfield is occupied. Bounds are the caller's problem;
    /// use [`Board::is_blocked`] for collision tests.
    #[inline]
    pub const fn occupied(&self, x: usize, y: usize) -> bool {
        self.rows[y] & (1 << x) != 0
    }

    /// Collision test in signed space, for pieces mid-kick.
    ///
    /// Out of bounds horizontally, or below the floor, is blocked — those are walls.
    /// Above the top of the buffer is **free**, so a kick that momentarily reaches above
    /// row 0 is legal.
    #[inline]
    pub const fn is_blocked(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= BOARD_W as i32 || y >= BOARD_H as i32 {
            return true;
        }
        if y < 0 {
            return false;
        }
        self.rows[y as usize] & (1 << x) != 0
    }

    #[inline]
    pub const fn is_row_full(&self, y: usize) -> bool {
        self.rows[y] == FULL_ROW
    }

    #[inline]
    pub const fn is_row_empty(&self, y: usize) -> bool {
        self.rows[y] == 0
    }

    /// Whether the whole playfield is empty — the perfect-clear test.
    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(|&r| r == 0)
    }

    /// Index of the highest occupied row, or `None` if the board is empty. Note that
    /// "highest" means smallest `y`.
    pub fn highest_occupied(&self) -> Option<usize> {
        self.rows.iter().position(|&r| r != 0)
    }

    /// Number of occupied cells. Used by the property tests to assert that cell count
    /// only ever changes in legal increments.
    pub fn cell_count(&self) -> u32 {
        self.rows.iter().map(|r| r.count_ones()).sum()
    }

    // ---- writes ------------------------------------------------------------

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, color: u8) {
        self.rows[y] |= 1 << x;
        self.colors[y * BOARD_W + x] = color;
    }

    #[inline]
    pub fn clear_cell(&mut self, x: usize, y: usize) {
        self.rows[y] &= !(1 << x);
        self.colors[y * BOARD_W + x] = COLOR_EMPTY;
    }

    /// Overwrite a whole row's occupancy. Test and tooling helper; the color channel is
    /// filled with `color` wherever a bit is set.
    pub fn set_row(&mut self, y: usize, mask: u16, color: u8) {
        self.rows[y] = mask & FULL_ROW;
        for x in 0..BOARD_W {
            self.colors[y * BOARD_W + x] = if mask & (1 << x) != 0 {
                color
            } else {
                COLOR_EMPTY
            };
        }
    }

    /// Remove every full row and collapse the rows above them downward.
    ///
    /// Returns the number of rows cleared. Occupancy and color move together, so the
    /// render channel can never drift out of alignment with the simulation.
    pub fn clear_full_rows(&mut self) -> u8 {
        let mut write = BOARD_H;
        for read in (0..BOARD_H).rev() {
            if self.rows[read] == FULL_ROW {
                continue;
            }
            write -= 1;
            if write != read {
                self.rows[write] = self.rows[read];
                self.colors
                    .copy_within(read * BOARD_W..(read + 1) * BOARD_W, write * BOARD_W);
            }
        }
        // `write` is now the count of cleared rows; blank that many rows at the top.
        for y in 0..write {
            self.rows[y] = 0;
        }
        self.colors[0..write * BOARD_W].fill(COLOR_EMPTY);
        write as u8
    }

    /// Push `amount` garbage rows in at the bottom, shifting the stack up.
    ///
    /// Returns `true` if occupied cells were pushed off the top of the buffer, which is
    /// a topout. The hole column is always supplied by the caller and is never derived
    /// from the engine's RNG stream, so that an authoritative peer can choose it from
    /// its own stream and both sides stay in agreement.
    pub fn push_garbage(&mut self, amount: usize, hole_col: usize) -> bool {
        debug_assert!(
            hole_col < BOARD_W,
            "hole column {hole_col} is off the board"
        );
        if amount == 0 {
            return false;
        }
        let amount = amount.min(BOARD_H);

        let overflow = self.rows[..amount].iter().any(|&r| r != 0);

        if amount < BOARD_H {
            self.rows.copy_within(amount.., 0);
            self.colors.copy_within(amount * BOARD_W.., 0);
        }

        let garbage_row = FULL_ROW & !(1 << hole_col);
        for y in (BOARD_H - amount)..BOARD_H {
            self.set_row(y, garbage_row, COLOR_GARBAGE);
        }
        overflow
    }

    // ---- tooling -----------------------------------------------------------

    /// Build a board from ASCII rows, bottom-anchored. `'.'`/`' '` is empty, anything
    /// else is occupied. Fixture helper for tests and input scripts.
    ///
    /// # Panics
    /// If any row is not exactly `BOARD_W` wide, or more than `BOARD_H` rows are given.
    pub fn from_ascii(rows: &[&str]) -> Self {
        assert!(rows.len() <= BOARD_H, "{} rows exceeds BOARD_H", rows.len());
        let mut b = Board::new();
        let top = BOARD_H - rows.len();
        for (i, line) in rows.iter().enumerate() {
            assert_eq!(
                line.chars().count(),
                BOARD_W,
                "row {i} is not {BOARD_W} wide"
            );
            for (x, ch) in line.chars().enumerate() {
                if ch != '.' && ch != ' ' {
                    b.set(x, top + i, COLOR_GARBAGE);
                }
            }
        }
        b
    }
}

/// Renders the occupied region as ASCII. The alternative view into a collision or kick
/// bug is a column of `u16` hex, so this earns its place in test failure output.
impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        let top = self.highest_occupied().unwrap_or(BOARD_H - 1);
        for y in top..BOARD_H {
            write!(f, "{y:2}|")?;
            for x in 0..BOARD_W {
                write!(f, "{}", if self.occupied(x, y) { '#' } else { '.' })?;
            }
            writeln!(f, "|")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_board_is_empty_everywhere() {
        let b = Board::new();
        assert!(b.is_empty());
        assert_eq!(b.cell_count(), 0);
        assert_eq!(b.highest_occupied(), None);
        for y in 0..BOARD_H {
            assert!(b.is_row_empty(y));
            assert!(!b.is_row_full(y));
        }
    }

    #[test]
    fn set_and_clear_a_single_cell() {
        let mut b = Board::new();
        b.set(3, 25, 4);
        assert!(b.occupied(3, 25));
        assert!(!b.occupied(4, 25));
        assert_eq!(b.colors()[25 * BOARD_W + 3], 4);
        assert_eq!(b.cell_count(), 1);

        b.clear_cell(3, 25);
        assert!(!b.occupied(3, 25));
        assert_eq!(b.colors()[25 * BOARD_W + 3], COLOR_EMPTY);
        assert!(b.is_empty());
    }

    #[test]
    fn walls_and_floor_block_but_above_the_buffer_is_free() {
        let b = Board::new();
        assert!(b.is_blocked(-1, 20), "left wall");
        assert!(b.is_blocked(BOARD_W as i32, 20), "right wall");
        assert!(b.is_blocked(5, BOARD_H as i32), "floor");
        assert!(
            !b.is_blocked(5, -1),
            "above the buffer must be free for kicks"
        );
        assert!(!b.is_blocked(0, 0));
        assert!(!b.is_blocked(BOARD_W as i32 - 1, BOARD_H as i32 - 1));
    }

    #[test]
    fn clearing_collapses_the_stack_and_preserves_color_alignment() {
        let mut b = Board::new();
        // Bottom row full, the row above it holding one distinctly-colored cell.
        b.set_row(BOARD_H - 1, FULL_ROW, 3);
        b.set(7, BOARD_H - 2, 5);

        assert_eq!(b.clear_full_rows(), 1);

        // The surviving cell fell exactly one row, color intact.
        assert!(!b.occupied(7, BOARD_H - 2));
        assert!(b.occupied(7, BOARD_H - 1));
        assert_eq!(b.colors()[(BOARD_H - 1) * BOARD_W + 7], 5);
        assert_eq!(b.cell_count(), 1);
    }

    #[test]
    fn clearing_handles_non_contiguous_full_rows() {
        let mut b = Board::new();
        b.set_row(BOARD_H - 1, FULL_ROW, 1); // full
        b.set_row(BOARD_H - 2, 0b1, 2); // survives
        b.set_row(BOARD_H - 3, FULL_ROW, 1); // full
        b.set_row(BOARD_H - 4, 0b10, 2); // survives

        assert_eq!(b.clear_full_rows(), 2);

        // The two survivors keep their relative order, resting on the floor.
        assert_eq!(b.row(BOARD_H - 1), 0b1);
        assert_eq!(b.row(BOARD_H - 2), 0b10);
        assert_eq!(b.cell_count(), 2);
    }

    #[test]
    fn clearing_a_full_board_empties_it() {
        let mut b = Board::new();
        for y in 0..BOARD_H {
            b.set_row(y, FULL_ROW, 1);
        }
        assert_eq!(b.clear_full_rows(), BOARD_H as u8);
        assert!(b.is_empty());
    }

    #[test]
    fn clearing_with_no_full_rows_changes_nothing() {
        let mut b = Board::new();
        b.set_row(BOARD_H - 1, 0b01_1111_1111, 1);
        let before = b.clone();
        assert_eq!(b.clear_full_rows(), 0);
        assert_eq!(b, before);
    }

    #[test]
    fn garbage_pushes_the_stack_up_and_leaves_exactly_one_hole() {
        let mut b = Board::new();
        b.set(0, BOARD_H - 1, 5);

        assert!(!b.push_garbage(2, 4));

        // The pre-existing cell rose by two rows.
        assert!(b.occupied(0, BOARD_H - 3));
        assert_eq!(b.colors()[(BOARD_H - 3) * BOARD_W], 5);

        for y in [BOARD_H - 1, BOARD_H - 2] {
            assert_eq!(b.row(y).count_ones(), BOARD_W as u32 - 1);
            assert!(!b.occupied(4, y), "hole column must stay open");
            assert_eq!(b.colors()[y * BOARD_W + 5], COLOR_GARBAGE);
            assert_eq!(b.colors()[y * BOARD_W + 4], COLOR_EMPTY);
        }
    }

    #[test]
    fn garbage_reports_overflow_when_it_pushes_cells_off_the_top() {
        let mut b = Board::new();
        b.set(0, 0, 5); // occupying the very top row
        assert!(b.push_garbage(1, 3), "should report a push-out");

        let mut b2 = Board::new();
        b2.set(0, BOARD_H - 1, 5);
        assert!(!b2.push_garbage(1, 3), "a low stack should not overflow");
    }

    #[test]
    fn zero_garbage_is_a_no_op() {
        let mut b = Board::new();
        b.set(2, BOARD_H - 1, 5);
        let before = b.clone();
        assert!(!b.push_garbage(0, 3));
        assert_eq!(b, before);
    }

    #[test]
    fn from_ascii_is_bottom_anchored() {
        let b = Board::from_ascii(&[
            "..........",
            "###.######", //
            "##########",
        ]);
        assert!(b.is_row_full(BOARD_H - 1));
        assert!(!b.occupied(3, BOARD_H - 2));
        assert!(b.occupied(2, BOARD_H - 2));
        assert!(b.is_row_empty(BOARD_H - 3));
        assert_eq!(b.cell_count(), 10 + 9);
    }

    #[test]
    fn ascii_fixture_round_trips_through_a_clear() {
        let mut b = Board::from_ascii(&["....#.....", "##########"]);
        assert_eq!(b.clear_full_rows(), 1);
        assert_eq!(b.cell_count(), 1);
        assert!(b.occupied(4, BOARD_H - 1));
    }
}
