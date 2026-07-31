//! Piece shapes and where they sit on the board.
//!
//! Each kind is four cells, given as offsets from the piece origin, for each of four
//! rotations. Spelling all four rotations out rather than rotating a matrix at runtime
//! keeps rotation free of arithmetic that could round differently anywhere, and makes
//! the shapes readable enough to check by eye.
//!
//! Offsets follow the standard 4x4 (or 3x3) box convention: `x` grows right and `y`
//! grows down, matching the board.

use crate::board::Board;
use crate::quad::{QuadKind, Rot};

/// A piece in play: what it is, which way up, and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub kind: QuadKind,
    pub rot: Rot,
    /// Column of the piece origin. Signed because a piece travels outside the board
    /// while a kick is being tried.
    pub x: i8,
    /// Row of the piece origin.
    pub y: i8,
}

impl Piece {
    pub const fn new(kind: QuadKind, rot: Rot, x: i8, y: i8) -> Self {
        Piece { kind, rot, x, y }
    }

    /// The four occupied cells, in board coordinates.
    pub fn cells(&self) -> [(i32, i32); 4] {
        let shape = SHAPES[self.kind.index()][self.rot.index()];
        let mut out = [(0, 0); 4];
        for (i, (dx, dy)) in shape.iter().enumerate() {
            out[i] = (self.x as i32 + *dx as i32, self.y as i32 + *dy as i32);
        }
        out
    }

    /// Whether this piece can occupy its current position.
    pub fn fits(&self, board: &Board) -> bool {
        self.cells().iter().all(|&(x, y)| !board.is_blocked(x, y))
    }

    /// The same piece shifted, if it fits there.
    pub fn moved(&self, dx: i8, dy: i8, board: &Board) -> Option<Piece> {
        let moved = Piece {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        };
        moved.fits(board).then_some(moved)
    }

    /// How far this piece can fall before something stops it.
    pub fn drop_distance(&self, board: &Board) -> i8 {
        let mut d = 0;
        while d < crate::consts::BOARD_H as i8 && self.moved(0, d + 1, board).is_some() {
            d += 1;
        }
        d
    }

    /// The piece where it would land if dropped straight down.
    pub fn landed(&self, board: &Board) -> Piece {
        Piece {
            y: self.y + self.drop_distance(board),
            ..*self
        }
    }

    /// Whether the piece is resting on the stack or the floor.
    pub fn is_grounded(&self, board: &Board) -> bool {
        self.moved(0, 1, board).is_none()
    }

    /// Whether the piece is boxed in on all four sides, which is how a spin is judged
    /// under the immobile rules.
    pub fn is_immobile(&self, board: &Board) -> bool {
        self.moved(0, 1, board).is_none()
            && self.moved(0, -1, board).is_none()
            && self.moved(-1, 0, board).is_none()
            && self.moved(1, 0, board).is_none()
    }
}

/// Cell offsets for every kind and rotation.
///
/// Rotations run clockwise from spawn. Written out in full: rotating a matrix at runtime
/// would be shorter but far harder to verify against a reference by reading it.
#[rustfmt::skip]
pub const SHAPES: [[[(i8, i8); 4]; 4]; QuadKind::COUNT] = [
    // I
    [
        [(0, 1), (1, 1), (2, 1), (3, 1)],
        [(2, 0), (2, 1), (2, 2), (2, 3)],
        [(0, 2), (1, 2), (2, 2), (3, 2)],
        [(1, 0), (1, 1), (1, 2), (1, 3)],
    ],
    // O
    [
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
    ],
    // T
    [
        [(1, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (1, 2)],
        [(1, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // S
    [
        [(1, 0), (2, 0), (0, 1), (1, 1)],
        [(1, 0), (1, 1), (2, 1), (2, 2)],
        [(1, 1), (2, 1), (0, 2), (1, 2)],
        [(0, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // Z
    [
        [(0, 0), (1, 0), (1, 1), (2, 1)],
        [(2, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (1, 2), (2, 2)],
        [(1, 0), (0, 1), (1, 1), (0, 2)],
    ],
    // J
    [
        [(0, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (2, 2)],
        [(1, 0), (1, 1), (0, 2), (1, 2)],
    ],
    // L
    [
        [(2, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (1, 2), (2, 2)],
        [(0, 1), (1, 1), (2, 1), (0, 2)],
        [(0, 0), (1, 0), (1, 1), (1, 2)],
    ],
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{BOARD_H, BOARD_W};

    #[test]
    fn every_shape_has_four_distinct_cells() {
        for kind in QuadKind::ALL {
            for rot in Rot::ALL {
                let cells = SHAPES[kind.index()][rot.index()];
                for (i, a) in cells.iter().enumerate() {
                    for b in &cells[i + 1..] {
                        assert_ne!(a, b, "{kind:?} {rot:?} has a duplicated cell");
                    }
                }
            }
        }
    }

    #[test]
    fn every_shape_is_connected() {
        // Four cells that are not orthogonally connected would not be a valid piece.
        for kind in QuadKind::ALL {
            for rot in Rot::ALL {
                let cells = SHAPES[kind.index()][rot.index()];
                let mut reached = [false; 4];
                reached[0] = true;
                for _ in 0..4 {
                    for i in 0..4 {
                        if !reached[i] {
                            continue;
                        }
                        for j in 0..4 {
                            let d =
                                (cells[i].0 - cells[j].0).abs() + (cells[i].1 - cells[j].1).abs();
                            if d == 1 {
                                reached[j] = true;
                            }
                        }
                    }
                }
                assert!(
                    reached.iter().all(|&r| r),
                    "{kind:?} {rot:?} is not connected"
                );
            }
        }
    }

    #[test]
    fn every_shape_fits_inside_a_four_wide_box() {
        for kind in QuadKind::ALL {
            for rot in Rot::ALL {
                for (dx, dy) in SHAPES[kind.index()][rot.index()] {
                    assert!((0..4).contains(&dx), "{kind:?} {rot:?} x={dx}");
                    assert!((0..4).contains(&dy), "{kind:?} {rot:?} y={dy}");
                }
            }
        }
    }

    #[test]
    fn o_is_the_same_in_every_rotation() {
        let base = SHAPES[QuadKind::O.index()][0];
        for rot in Rot::ALL {
            assert_eq!(SHAPES[QuadKind::O.index()][rot.index()], base);
        }
    }

    #[test]
    fn half_turn_symmetric_kinds_have_congruent_opposite_rotations() {
        // I, S and Z look the same after a 180 turn, so each rotation and its opposite
        // must be the same shape up to a shift. Comparing them normalised to the origin
        // is what makes that a real check rather than a coincidence of offsets.
        for kind in [QuadKind::I, QuadKind::S, QuadKind::Z] {
            for rot in Rot::ALL {
                let a = normalised(SHAPES[kind.index()][rot.index()]);
                let b = normalised(SHAPES[kind.index()][rot.flip().index()]);
                assert_eq!(a, b, "{kind:?} {rot:?} differs from its opposite");
            }
        }
    }

    /// Sort a shape and shift it so its top-left corner sits at the origin, so that two
    /// shapes can be compared regardless of where they sit in their box.
    fn normalised(cells: [(i8, i8); 4]) -> [(i8, i8); 4] {
        let min_x = cells.iter().map(|c| c.0).min().unwrap();
        let min_y = cells.iter().map(|c| c.1).min().unwrap();
        let mut out = cells.map(|(x, y)| (x - min_x, y - min_y));
        out.sort_unstable();
        out
    }

    fn sorted(mut cells: [(i8, i8); 4]) -> [(i8, i8); 4] {
        cells.sort_unstable();
        cells
    }

    #[test]
    fn rotations_of_a_kind_are_all_different_except_for_o() {
        for kind in QuadKind::ALL {
            if kind == QuadKind::O {
                continue;
            }
            let r0 = sorted(SHAPES[kind.index()][0]);
            let r1 = sorted(SHAPES[kind.index()][1]);
            assert_ne!(r0, r1, "{kind:?} does not change when rotated");
        }
    }

    #[test]
    fn a_piece_reports_its_cells_relative_to_its_position() {
        let mut cells = Piece::new(QuadKind::O, Rot::R0, 4, 20).cells();
        cells.sort_unstable();
        assert_eq!(cells, [(5, 20), (5, 21), (6, 20), (6, 21)]);
    }

    #[test]
    fn a_piece_in_open_space_fits() {
        let board = Board::new();
        assert!(Piece::new(QuadKind::T, Rot::R0, 3, 20).fits(&board));
    }

    #[test]
    fn a_piece_overlapping_the_stack_does_not_fit() {
        let mut board = Board::new();
        board.set(4, 21, 1);
        assert!(!Piece::new(QuadKind::O, Rot::R0, 3, 20).fits(&board));
    }

    #[test]
    fn a_piece_cannot_leave_the_board_sideways() {
        let board = Board::new();
        assert!(!Piece::new(QuadKind::T, Rot::R0, -2, 20).fits(&board));
        assert!(!Piece::new(QuadKind::T, Rot::R0, BOARD_W as i8, 20).fits(&board));
    }

    #[test]
    fn a_piece_falls_to_the_floor_of_an_empty_board() {
        let board = Board::new();
        let p = Piece::new(QuadKind::O, Rot::R0, 3, 0);
        let landed = p.landed(&board);
        // The O occupies rows y and y+1, so its origin rests one row above the floor.
        assert_eq!(landed.y as usize, BOARD_H - 2);
        assert!(landed.is_grounded(&board));
    }

    #[test]
    fn a_piece_lands_on_top_of_the_stack() {
        let mut board = Board::new();
        for x in 0..BOARD_W {
            board.set(x, BOARD_H - 1, 1);
        }
        let landed = Piece::new(QuadKind::O, Rot::R0, 3, 0).landed(&board);
        assert_eq!(landed.y as usize, BOARD_H - 3);
    }

    #[test]
    fn a_grounded_piece_reports_zero_drop_distance() {
        let board = Board::new();
        let landed = Piece::new(QuadKind::T, Rot::R0, 3, 0).landed(&board);
        assert_eq!(landed.drop_distance(&board), 0);
        assert!(landed.is_grounded(&board));
    }

    #[test]
    fn moving_into_a_wall_returns_nothing() {
        let board = Board::new();
        let p = Piece::new(QuadKind::O, Rot::R0, -1, 20);
        assert!(p.moved(-1, 0, &board).is_none());
        assert!(p.moved(1, 0, &board).is_some());
    }

    #[test]
    fn a_piece_in_open_space_is_not_immobile() {
        let board = Board::new();
        assert!(!Piece::new(QuadKind::T, Rot::R0, 3, 20).is_immobile(&board));
    }

    #[test]
    fn a_piece_boxed_in_on_every_side_is_immobile() {
        // A T sitting in a slot exactly its own shape: this is what a spin looks like.
        let mut board = Board::new();
        let p = Piece::new(QuadKind::T, Rot::R0, 3, BOARD_H as i8 - 2);
        let occupied: Vec<(i32, i32)> = p.cells().to_vec();
        for y in (BOARD_H - 2)..BOARD_H {
            for x in 0..BOARD_W {
                if !occupied.contains(&(x as i32, y as i32)) {
                    board.set(x, y, 1);
                }
            }
        }
        assert!(p.fits(&board));
        assert!(p.is_immobile(&board));
    }
}
