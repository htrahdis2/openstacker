//! Rotation, and the wall kicks that make it work in tight spaces.
//!
//! A rotation is tried at a list of offsets in order, and the first that fits wins. That
//! list is what lets a piece twist into a slot it could not simply be dropped into, and
//! it is the single most feel-defining table in the game.
//!
//! Which offset succeeded is reported back, because the last kick in a list is the
//! extreme one and is what distinguishes a full spin from a mini.

use crate::board::Board;
use crate::piece::Piece;
use crate::quad::{QuadKind, Rot};

/// Result of a rotation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kicked {
    pub piece: Piece,
    /// Index into the offset list that succeeded. `0` is the plain rotation with no
    /// displacement; the last index is the most extreme kick available.
    pub kick_index: u8,
}

/// Number of offsets tried for a quarter turn.
pub const QUARTER_KICKS: usize = 5;
/// Number of offsets tried for a half turn.
pub const HALF_KICKS: usize = 6;

/// Try to rotate a piece, kicking if the plain rotation does not fit.
///
/// Returns `None` when every offset is blocked, which leaves the piece where it was.
pub fn rotate(piece: &Piece, to: Rot, board: &Board) -> Option<Kicked> {
    if piece.rot == to {
        return None;
    }
    let offsets = offsets_for(piece.kind, piece.rot, to);

    for (i, &(dx, dy)) in offsets.iter().enumerate() {
        // Table offsets use y-up, matching how the reference tables are published.
        // The board is y-down, so the vertical component is negated exactly here and
        // nowhere else.
        let candidate = Piece {
            rot: to,
            x: piece.x + dx,
            y: piece.y - dy,
            ..*piece
        };
        if candidate.fits(board) {
            return Some(Kicked {
                piece: candidate,
                kick_index: i as u8,
            });
        }
    }
    None
}

/// The offsets to try for a given kind and rotation change.
pub fn offsets_for(kind: QuadKind, from: Rot, to: Rot) -> &'static [(i8, i8)] {
    if !kind.kicks() {
        return &NO_KICK;
    }
    if from.flip() == to {
        return &HALF[half_index(from)];
    }
    let i = quarter_index(from, to);
    match kind {
        QuadKind::I => &KICKS_I[i],
        _ => &KICKS_JLSTZ[i],
    }
}

/// O never displaces, so it has a single no-op offset.
const NO_KICK: [(i8, i8); 1] = [(0, 0)];

/// Index of a quarter turn in the kick tables.
///
/// The eight quarter turns are ordered 0>1, 1>0, 1>2, 2>1, 2>3, 3>2, 3>0, 0>3.
fn quarter_index(from: Rot, to: Rot) -> usize {
    match (from.index(), to.index()) {
        (0, 1) => 0,
        (1, 0) => 1,
        (1, 2) => 2,
        (2, 1) => 3,
        (2, 3) => 4,
        (3, 2) => 5,
        (3, 0) => 6,
        (0, 3) => 7,
        _ => unreachable!("not a quarter turn"),
    }
}

/// Index of a half turn: 0>2, 1>3, 2>0, 3>1.
fn half_index(from: Rot) -> usize {
    from.index()
}

/// Kick offsets for J, L, S, T and Z. Published y-up.
#[rustfmt::skip]
pub const KICKS_JLSTZ: [[(i8, i8); QUARTER_KICKS]; 8] = [
    [(0, 0), (-1, 0), (-1,  1), (0, -2), (-1, -2)], // 0>1
    [(0, 0), ( 1, 0), ( 1, -1), (0,  2), ( 1,  2)], // 1>0
    [(0, 0), ( 1, 0), ( 1, -1), (0,  2), ( 1,  2)], // 1>2
    [(0, 0), (-1, 0), (-1,  1), (0, -2), (-1, -2)], // 2>1
    [(0, 0), ( 1, 0), ( 1,  1), (0, -2), ( 1, -2)], // 2>3
    [(0, 0), (-1, 0), (-1, -1), (0,  2), (-1,  2)], // 3>2
    [(0, 0), (-1, 0), (-1, -1), (0,  2), (-1,  2)], // 3>0
    [(0, 0), ( 1, 0), ( 1,  1), (0, -2), ( 1, -2)], // 0>3
];

/// Kick offsets for I, which needs its own table because it pivots differently.
#[rustfmt::skip]
pub const KICKS_I: [[(i8, i8); QUARTER_KICKS]; 8] = [
    [(0, 0), (-2, 0), ( 1, 0), (-2, -1), ( 1,  2)], // 0>1
    [(0, 0), ( 2, 0), (-1, 0), ( 2,  1), (-1, -2)], // 1>0
    [(0, 0), (-1, 0), ( 2, 0), (-1,  2), ( 2, -1)], // 1>2
    [(0, 0), ( 1, 0), (-2, 0), ( 1, -2), (-2,  1)], // 2>1
    [(0, 0), ( 2, 0), (-1, 0), ( 2,  1), (-1, -2)], // 2>3
    [(0, 0), (-2, 0), ( 1, 0), (-2, -1), ( 1,  2)], // 3>2
    [(0, 0), ( 1, 0), (-2, 0), ( 1, -2), (-2,  1)], // 3>0
    [(0, 0), (-1, 0), ( 2, 0), (-1,  2), ( 2, -1)], // 0>3
];

/// Half-turn kicks, indexed by the rotation being left.
///
/// Half turns are not part of classic SRS. These are the widely used extension: mostly
/// vertical, with a sideways pair last so a flip can escape a wall.
#[rustfmt::skip]
pub const HALF: [[(i8, i8); HALF_KICKS]; 4] = [
    [(0, 0), (0,  1), (1,  1), (-1,  1), ( 1, 0), (-1, 0)], // 0>2
    [(0, 0), (1,  0), (1,  2), ( 1,  1), ( 0, 2), ( 0, 1)], // 1>3
    [(0, 0), (0, -1), (-1, -1), (1, -1), (-1, 0), ( 1, 0)], // 2>0
    [(0, 0), (-1, 0), (-1, 2), (-1, 1), ( 0, 2), ( 0, 1)], // 3>1
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{BOARD_H, BOARD_W};

    fn empty() -> Board {
        Board::new()
    }

    #[test]
    fn every_kick_list_starts_with_the_plain_rotation() {
        // Index 0 must be the no-displacement case, or a piece would drift sideways
        // every time it rotated in open space.
        for table in [&KICKS_JLSTZ, &KICKS_I] {
            for row in table.iter() {
                assert_eq!(row[0], (0, 0));
            }
        }
        for row in HALF.iter() {
            assert_eq!(row[0], (0, 0));
        }
    }

    #[test]
    fn a_quarter_turn_and_its_reverse_are_mirror_images() {
        // If they were not, rotating one way and back would not return a piece to where
        // it started, which players would feel immediately.
        for table in [&KICKS_JLSTZ, &KICKS_I] {
            for pair in [(0, 1), (2, 3), (4, 5), (6, 7)] {
                let (a, b) = (table[pair.0], table[pair.1]);
                for i in 0..QUARTER_KICKS {
                    assert_eq!(
                        (a[i].0, a[i].1),
                        (-b[i].0, -b[i].1),
                        "row {} offset {i} is not the mirror of row {}",
                        pair.0,
                        pair.1
                    );
                }
            }
        }
    }

    #[test]
    fn every_quarter_turn_maps_to_a_distinct_table_row() {
        let mut seen = [false; 8];
        for from in Rot::ALL {
            for to in [from.cw(), from.ccw()] {
                let i = quarter_index(from, to);
                assert!(!seen[i], "two turns share row {i}");
                seen[i] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "some rows are unreachable");
    }

    #[test]
    fn o_never_displaces() {
        for from in Rot::ALL {
            for to in Rot::ALL {
                if from == to {
                    continue;
                }
                assert_eq!(offsets_for(QuadKind::O, from, to), &NO_KICK);
            }
        }
    }

    #[test]
    fn i_uses_its_own_table() {
        assert_eq!(
            offsets_for(QuadKind::I, Rot::R0, Rot::R1),
            &KICKS_I[0][..],
            "I must not share the JLSTZ table"
        );
        assert_eq!(
            offsets_for(QuadKind::T, Rot::R0, Rot::R1),
            &KICKS_JLSTZ[0][..]
        );
    }

    #[test]
    fn a_half_turn_uses_the_half_table() {
        for from in Rot::ALL {
            let offsets = offsets_for(QuadKind::T, from, from.flip());
            assert_eq!(offsets.len(), HALF_KICKS);
        }
    }

    #[test]
    fn rotating_in_open_space_never_kicks() {
        let board = empty();
        for kind in QuadKind::ALL {
            for from in Rot::ALL {
                let p = Piece::new(kind, from, 3, 20);
                if !p.fits(&board) {
                    continue;
                }
                for to in [from.cw(), from.ccw(), from.flip()] {
                    let k = rotate(&p, to, &board).expect("open space rotation must work");
                    assert_eq!(
                        k.kick_index, 0,
                        "{kind:?} {from:?}->{to:?} kicked in open space"
                    );
                    assert_eq!((k.piece.x, k.piece.y), (p.x, p.y));
                }
            }
        }
    }

    #[test]
    fn rotating_to_the_same_rotation_does_nothing() {
        let board = empty();
        let p = Piece::new(QuadKind::T, Rot::R0, 3, 20);
        assert!(rotate(&p, Rot::R0, &board).is_none());
    }

    #[test]
    fn a_piece_against_the_left_wall_kicks_away_from_it() {
        // The whole purpose of the table. A vertical I hard against the left wall has
        // no room to become horizontal in place, so the rotation is displaced onto the
        // board instead of being refused.
        let board = empty();
        let p = Piece::new(QuadKind::I, Rot::R1, -2, 20);
        assert!(p.fits(&board), "the vertical I should sit in column 0");

        let k = rotate(&p, Rot::R0, &board).expect("should kick off the wall");
        assert!(k.piece.fits(&board));
        assert_ne!(k.kick_index, 0, "the plain rotation should not have fitted");
        assert_eq!(k.piece.x, 0, "displaced right, onto the board");

        let mut cells = k.piece.cells();
        cells.sort_unstable();
        assert_eq!(cells, [(0, 21), (1, 21), (2, 21), (3, 21)]);
    }

    #[test]
    fn a_piece_against_the_right_wall_kicks_away_from_it() {
        let board = empty();
        let p = Piece::new(QuadKind::I, Rot::R1, BOARD_W as i8 - 3, 20);
        assert!(
            p.fits(&board),
            "the vertical I should sit in the last column"
        );

        let k = rotate(&p, Rot::R0, &board).expect("should kick off the wall");
        assert!(k.piece.fits(&board));
        assert_ne!(k.kick_index, 0);
        for (x, _) in k.piece.cells() {
            assert!((0..BOARD_W as i32).contains(&x), "kicked off the board");
        }
    }

    #[test]
    fn rotation_is_refused_when_every_offset_is_blocked() {
        // Fully packed board: nothing can move, and rotation must report that rather
        // than teleporting the piece somewhere it does not belong.
        let mut board = Board::new();
        for y in 0..BOARD_H {
            for x in 0..BOARD_W {
                board.set(x, y, 1);
            }
        }
        let p = Piece::new(QuadKind::T, Rot::R0, 3, 20);
        assert!(rotate(&p, Rot::R1, &board).is_none());
    }

    #[test]
    fn a_successful_rotation_always_lands_somewhere_legal() {
        // Exhaustive over kinds, rotations, and positions on a scattered board. A kick
        // that returned an overlapping piece would corrupt the board on the next lock.
        let mut board = Board::new();
        for (i, y) in (20..BOARD_H).enumerate() {
            board.set_row(y, ((i * 37) % 1024) as u16, 1);
        }
        let mut kicked = 0;
        for kind in QuadKind::ALL {
            for from in Rot::ALL {
                for x in -3..(BOARD_W as i8 + 3) {
                    for y in 0..(BOARD_H as i8 - 1) {
                        let p = Piece::new(kind, from, x, y);
                        if !p.fits(&board) {
                            continue;
                        }
                        for to in [from.cw(), from.ccw(), from.flip()] {
                            if let Some(k) = rotate(&p, to, &board) {
                                assert!(
                                    k.piece.fits(&board),
                                    "{kind:?} {from:?}->{to:?} at ({x},{y}) landed illegally"
                                );
                                assert_eq!(k.piece.rot, to);
                                if k.kick_index > 0 {
                                    kicked += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(kicked > 0, "the scenario never exercised an actual kick");
    }

    #[test]
    fn a_kick_index_is_always_within_its_table() {
        let mut board = Board::new();
        board.set_row(BOARD_H - 1, 0b11_1100_0011, 1);
        board.set_row(BOARD_H - 2, 0b11_0000_0011, 1);
        for kind in QuadKind::ALL {
            for from in Rot::ALL {
                for x in -2..(BOARD_W as i8) {
                    let p = Piece::new(kind, from, x, BOARD_H as i8 - 4);
                    if !p.fits(&board) {
                        continue;
                    }
                    for to in [from.cw(), from.ccw(), from.flip()] {
                        if let Some(k) = rotate(&p, to, &board) {
                            let len = offsets_for(kind, from, to).len();
                            assert!((k.kick_index as usize) < len);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rotating_four_times_returns_a_piece_to_where_it_started() {
        // True only in open space, where no kick displaces anything, but it is the
        // clearest statement that the shape tables and rotation agree with each other.
        let board = empty();
        for kind in QuadKind::ALL {
            let start = Piece::new(kind, Rot::R0, 3, 18);
            if !start.fits(&board) {
                continue;
            }
            let mut p = start;
            for _ in 0..4 {
                p = rotate(&p, p.rot.cw(), &board).expect("open rotation").piece;
            }
            assert_eq!(p, start, "{kind:?} did not return to its start");
        }
    }
}
