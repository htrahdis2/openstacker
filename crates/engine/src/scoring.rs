//! Judging what a clear was worth.
//!
//! Two questions, in order: was the piece spun into place, and how many rows did that
//! clear. Together with the combo and back-to-back state they decide how many rows are
//! sent to an opponent.

use crate::board::Board;
use crate::config::match_config::{MatchConfig, SpinRule};
use crate::kick::QUARTER_KICKS;
use crate::piece::Piece;
use crate::quad::QuadKind;

/// Whether a lock counted as a spin, and how strongly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Spin {
    #[default]
    None,
    /// A spin into a shallow pocket. Worth less than a full one.
    Mini,
    Full,
}

impl Spin {
    pub const fn is_spin(self) -> bool {
        !matches!(self, Spin::None)
    }
}

/// The four corners of a T's bounding box, relative to the piece origin.
const T_CORNERS: [(i8, i8); 4] = [(0, 0), (2, 0), (0, 2), (2, 2)];

/// Which two corners a T faces in each rotation.
///
/// Both being filled means the piece was driven into a proper pocket rather than
/// resting against its own flat back, which is the difference between a full spin and a
/// mini.
const T_FRONT: [[(i8, i8); 2]; 4] = [
    [(0, 0), (2, 0)], // pointing up
    [(2, 0), (2, 2)], // pointing right
    [(0, 2), (2, 2)], // pointing down
    [(0, 0), (0, 2)], // pointing left
];

/// Judge whether a lock was a spin.
///
/// `last_action_was_rotation` is what stops a piece that merely slid into a gap from
/// scoring as a spin. `kick_index` promotes a mini to a full spin when the piece got
/// there by the most extreme kick available, because that displacement is only reachable
/// deliberately.
pub fn detect_spin(
    piece: &Piece,
    board: &Board,
    rule: SpinRule,
    last_action_was_rotation: bool,
    kick_index: u8,
) -> Spin {
    if !last_action_was_rotation {
        return Spin::None;
    }
    match rule {
        SpinRule::None => Spin::None,
        SpinRule::ThreeCorner => {
            if piece.kind != QuadKind::T {
                return Spin::None;
            }
            three_corner(piece, board, kick_index)
        }
        SpinRule::Immobile => {
            if piece.kind != QuadKind::T {
                return Spin::None;
            }
            immobile(piece, board, kick_index)
        }
        SpinRule::AllSpin => immobile(piece, board, kick_index),
    }
}

fn three_corner(piece: &Piece, board: &Board, kick_index: u8) -> Spin {
    let filled = |(dx, dy): (i8, i8)| {
        board.is_blocked(piece.x as i32 + dx as i32, piece.y as i32 + dy as i32)
    };
    let corners = T_CORNERS.iter().filter(|&&c| filled(c)).count();
    if corners < 3 {
        return Spin::None;
    }
    let front = T_FRONT[piece.rot.index()];
    let both_front = front.iter().all(|&c| filled(c));
    if both_front || kick_index as usize >= QUARTER_KICKS - 1 {
        Spin::Full
    } else {
        Spin::Mini
    }
}

fn immobile(piece: &Piece, board: &Board, kick_index: u8) -> Spin {
    if !piece.is_immobile(board) {
        return Spin::None;
    }
    if kick_index as usize >= QUARTER_KICKS - 1 {
        Spin::Full
    } else {
        Spin::Mini
    }
}

/// Whether a clear keeps a back-to-back chain alive.
///
/// Quads and spins are hard to set up, so chaining them is rewarded. Plain clears of one
/// to three rows break the chain.
pub const fn continues_b2b(lines: u8, spin: Spin) -> bool {
    lines >= 4 || spin.is_spin()
}

/// Rows sent for a clear, before cancellation.
///
/// `b2b_chain` is how long the back-to-back run was *before* this clear. The bonus is
/// paid only when this clear carries the chain on, so the plain clear that ends a run
/// does not collect on a chain it just broke.
pub fn attack_for(
    lines: u8,
    spin: Spin,
    perfect_clear: bool,
    b2b_chain: u8,
    combo: u8,
    config: &MatchConfig,
) -> u8 {
    if lines == 0 {
        return 0;
    }
    let t = &config.attack_table;
    let base = match (spin, lines) {
        (Spin::Full, 1) => t.spin_single,
        (Spin::Full, 2) => t.spin_double,
        (Spin::Full, 3) => t.spin_triple,
        (Spin::Full, n) if n >= 4 => t.spin_quad,
        (Spin::Mini, 1) => t.mini_spin_single,
        (Spin::Mini, n) if n >= 2 => t.mini_spin_double,
        (Spin::None, 1) => t.single,
        (Spin::None, 2) => t.double,
        (Spin::None, 3) => t.triple,
        (Spin::None, n) if n >= 4 => t.quad,
        _ => 0,
    };

    let mut total = base as u32;
    total += config.combo_bonus(combo) as u32;
    if continues_b2b(lines, spin) {
        total += config.b2b_bonus(b2b_chain) as u32;
    }
    if perfect_clear {
        total += t.perfect_clear as u32;
    }
    total.min(u8::MAX as u32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{BOARD_H, BOARD_W};
    use crate::quad::Rot;

    /// A T in a pocket with three corners filled, pointing down into it.
    fn t_spin_board() -> (Board, Piece) {
        let mut board = Board::new();
        let y = BOARD_H as i8 - 3;
        let piece = Piece::new(QuadKind::T, Rot::R2, 3, y);
        let occupied: Vec<(i32, i32)> = piece.cells().to_vec();
        for row in (BOARD_H - 3)..BOARD_H {
            for x in 0..BOARD_W {
                if !occupied.contains(&(x as i32, row as i32)) {
                    board.set(x, row, 1);
                }
            }
        }
        (board, piece)
    }

    #[test]
    fn a_slide_into_a_gap_is_not_a_spin() {
        // The defining rule. Without it, dropping a piece into a hole would score the
        // same as rotating it in, and spins would stop being a skill.
        let (board, piece) = t_spin_board();
        assert_eq!(
            detect_spin(&piece, &board, SpinRule::ThreeCorner, false, 0),
            Spin::None
        );
    }

    #[test]
    fn a_rotation_into_a_three_corner_pocket_is_a_spin() {
        let (board, piece) = t_spin_board();
        assert!(detect_spin(&piece, &board, SpinRule::ThreeCorner, true, 0).is_spin());
    }

    #[test]
    fn the_three_corner_rule_ignores_kinds_other_than_t() {
        let (board, mut piece) = t_spin_board();
        piece.kind = QuadKind::L;
        assert_eq!(
            detect_spin(&piece, &board, SpinRule::ThreeCorner, true, 0),
            Spin::None
        );
    }

    #[test]
    fn an_open_board_yields_no_spin() {
        let board = Board::new();
        let piece = Piece::new(QuadKind::T, Rot::R0, 3, 20);
        assert_eq!(
            detect_spin(&piece, &board, SpinRule::ThreeCorner, true, 0),
            Spin::None
        );
    }

    #[test]
    fn spin_detection_can_be_turned_off_entirely() {
        let (board, piece) = t_spin_board();
        assert_eq!(
            detect_spin(&piece, &board, SpinRule::None, true, 4),
            Spin::None
        );
    }

    #[test]
    fn the_most_extreme_kick_promotes_a_mini_to_a_full_spin() {
        // The last offset in a kick list is a large displacement that cannot be reached
        // by accident, so arriving through it is treated as the real thing.
        let (board, piece) = t_spin_board();
        let plain = detect_spin(&piece, &board, SpinRule::Immobile, true, 0);
        let kicked = detect_spin(&piece, &board, SpinRule::Immobile, true, 4);
        assert_eq!(kicked, Spin::Full);
        assert!(plain.is_spin());
    }

    #[test]
    fn all_spin_recognises_kinds_the_t_only_rules_reject() {
        let mut board = Board::new();
        let piece = Piece::new(QuadKind::L, Rot::R0, 3, BOARD_H as i8 - 2);
        let occupied: Vec<(i32, i32)> = piece.cells().to_vec();
        for row in (BOARD_H - 2)..BOARD_H {
            for x in 0..BOARD_W {
                if !occupied.contains(&(x as i32, row as i32)) {
                    board.set(x, row, 1);
                }
            }
        }
        assert!(
            piece.is_immobile(&board),
            "the fixture should box the piece in"
        );
        assert_eq!(
            detect_spin(&piece, &board, SpinRule::Immobile, true, 0),
            Spin::None,
            "the T-only rule must ignore an L"
        );
        assert!(detect_spin(&piece, &board, SpinRule::AllSpin, true, 0).is_spin());
    }

    // ---- attack ------------------------------------------------------------

    fn cfg() -> MatchConfig {
        MatchConfig::default()
    }

    #[test]
    fn clearing_nothing_sends_nothing() {
        assert_eq!(attack_for(0, Spin::None, false, 0, 0, &cfg()), 0);
    }

    #[test]
    fn plain_clears_follow_the_table() {
        let c = cfg();
        assert_eq!(attack_for(1, Spin::None, false, 0, 0, &c), 0);
        assert_eq!(attack_for(2, Spin::None, false, 0, 0, &c), 1);
        assert_eq!(attack_for(3, Spin::None, false, 0, 0, &c), 2);
        assert_eq!(attack_for(4, Spin::None, false, 0, 0, &c), 4);
    }

    #[test]
    fn a_spin_beats_the_plain_clear_of_the_same_size() {
        // The entire reason to learn spins. If this ever inverts, the skill stops
        // paying for itself.
        let c = cfg();
        for lines in 1..=3u8 {
            let plain = attack_for(lines, Spin::None, false, 0, 0, &c);
            let spun = attack_for(lines, Spin::Full, false, 0, 0, &c);
            assert!(spun > plain, "{lines} rows: spin {spun} vs plain {plain}");
        }
    }

    #[test]
    fn a_full_spin_beats_a_mini_of_the_same_size() {
        let c = cfg();
        for lines in 1..=2u8 {
            assert!(
                attack_for(lines, Spin::Full, false, 0, 0, &c)
                    > attack_for(lines, Spin::Mini, false, 0, 0, &c)
            );
        }
    }

    #[test]
    fn back_to_back_adds_its_bonus() {
        let c = cfg();
        let without = attack_for(4, Spin::None, false, 0, 0, &c);
        let with = attack_for(4, Spin::None, false, 1, 0, &c);
        assert_eq!(with - without, c.b2b_bonus(1));
    }

    #[test]
    fn a_longer_chain_pays_more_than_a_short_one() {
        // The reason back-to-back is a table rather than one number: a chain that has
        // been kept alive is worth more than one that just started.
        let c = cfg();
        let short = attack_for(4, Spin::None, false, 1, 0, &c);
        let long = attack_for(4, Spin::None, false, 8, 0, &c);
        assert!(long > short, "chain of 8 {long} vs chain of 1 {short}");
    }

    #[test]
    fn the_chain_reward_saturates_past_the_end_of_the_table() {
        let c = cfg();
        let last = *c.b2b_table.last().unwrap();
        assert_eq!(c.b2b_bonus(u8::MAX), last);
        assert_eq!(c.b2b_bonus(0), 0, "no chain yet is worth nothing");
    }

    #[test]
    fn the_clear_that_breaks_a_chain_does_not_collect_on_it() {
        // A plain double after a run of quads ends the run. Paying it the chain bonus
        // would reward breaking the thing the bonus exists to encourage.
        let c = cfg();
        let breaking = attack_for(2, Spin::None, false, 5, 0, &c);
        let no_chain = attack_for(2, Spin::None, false, 0, 0, &c);
        assert_eq!(breaking, no_chain);
    }

    #[test]
    fn a_spin_quad_is_not_scored_as_a_spin_triple() {
        // Only reachable under all-spin rules, where it would otherwise be the biggest
        // clear in the game and worth less than the mode intends.
        let c = cfg();
        assert_eq!(
            attack_for(4, Spin::Full, false, 0, 0, &c),
            c.attack_table.spin_quad
        );
        assert!(c.attack_table.spin_quad > c.attack_table.spin_triple);
    }

    #[test]
    fn a_perfect_clear_adds_its_bonus() {
        let c = cfg();
        let without = attack_for(4, Spin::None, false, 0, 0, &c);
        let with = attack_for(4, Spin::None, true, 0, 0, &c);
        assert_eq!(with - without, c.attack_table.perfect_clear);
    }

    #[test]
    fn combo_adds_to_the_total_and_never_indexes_past_the_table() {
        let c = cfg();
        let base = attack_for(2, Spin::None, false, 0, 0, &c);
        assert!(attack_for(2, Spin::None, false, 0, 5, &c) > base);
        // Far past the end of the table: must saturate rather than panic.
        let _ = attack_for(2, Spin::None, false, 0, u8::MAX, &c);
    }

    #[test]
    fn attack_saturates_rather_than_wrapping() {
        // Combo table entries have no per-entry bound, so a strange mode can push the
        // total past what a u8 holds. Wrapping would turn an enormous attack into a tiny
        // one, which is a far more confusing bug than a capped one.
        let mut combo_table = arrayvec::ArrayVec::new();
        combo_table.push(u8::MAX);
        let c = MatchConfig {
            attack_table: crate::AttackTable {
                quad: 40,
                perfect_clear: 40,
                ..Default::default()
            },
            combo_table,
            ..Default::default()
        };
        // 40 + 255 + 20 + 40 overflows a u8 several times over.
        assert_eq!(attack_for(4, Spin::None, true, 1, 0, &c), u8::MAX);
    }

    #[test]
    fn ordinary_play_never_reaches_the_cap() {
        // If a normal quad were already saturating, the cap would be silently flattening
        // real differences in attack rather than guarding an edge case.
        let c = cfg();
        assert!(attack_for(4, Spin::Full, true, 1, 12, &c) < u8::MAX);
    }

    #[test]
    fn quads_and_spins_carry_a_chain_while_plain_clears_break_it() {
        assert!(continues_b2b(4, Spin::None));
        assert!(continues_b2b(1, Spin::Full));
        assert!(continues_b2b(1, Spin::Mini));
        assert!(!continues_b2b(1, Spin::None));
        assert!(!continues_b2b(3, Spin::None));
    }
}
