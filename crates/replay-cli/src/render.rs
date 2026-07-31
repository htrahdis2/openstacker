//! Drawing the board as text.
//!
//! Without a graphical client, the alternative view into a rotation or collision bug is
//! a column of hex. This is forty lines and no dependencies, and it is the difference
//! between reading bitmasks and seeing the piece in the wrong place.

use engine::{BOARD_H, BOARD_W, Engine, VISIBLE_H};
use std::fmt::Write;

/// Render the board, the active piece, its landing position, and the run's counters.
pub fn render(engine: &Engine) -> String {
    let board = engine.board();
    // There is no piece during a spawn or clear delay, or once the game is over.
    let active: Vec<(i32, i32)> = engine
        .active()
        .map(|p| p.cells().to_vec())
        .unwrap_or_default();
    let ghost: Vec<(i32, i32)> = engine
        .ghost()
        .map(|p| p.cells().to_vec())
        .unwrap_or_default();

    // Show the visible field, plus any buffer rows that are actually in use, so a stack
    // pushing into the buffer is visible rather than silently cut off.
    let highest = board.highest_occupied().unwrap_or(BOARD_H - 1);
    let piece_top = active.iter().map(|c| c.1).min().unwrap_or(0).max(0) as usize;
    let top = highest.min(piece_top).min(BOARD_H - VISIBLE_H);

    let stats = engine.stats();
    let mut out = String::new();
    let piece = match engine.active() {
        Some(p) => format!("{}{:?}", p.kind.label(), p.rot),
        None => "-".to_string(),
    };
    let _ = writeln!(
        out,
        "tick {}  piece {piece}  phase {:?}",
        stats.tick,
        engine.phase()
    );

    let preview: String = engine.preview().iter().map(|k| k.label()).collect();
    let hold = engine.hold().map_or('-', |k| k.label());

    let _ = writeln!(out, "  +{}+", "-".repeat(BOARD_W));
    for y in top..BOARD_H {
        let _ = write!(out, "{y:2}|");
        for x in 0..BOARD_W {
            let cell = (x as i32, y as i32);
            let ch = if active.contains(&cell) {
                '@'
            } else if board.occupied(x, y) {
                '#'
            } else if ghost.contains(&cell) {
                ':'
            } else if y < VISIBLE_H {
                // Buffer rows are dotted differently so it is obvious when a stack has
                // pushed above the playfield.
                ','
            } else {
                '.'
            };
            let _ = out.write_char(ch);
        }
        let _ = write!(out, "|");

        // Annotate the first few rows with the run's counters.
        match y - top {
            0 => {
                let _ = write!(out, "  next {preview}");
            }
            1 => {
                let _ = write!(out, "  hold {hold}");
            }
            2 => {
                let _ = write!(out, "  lines {}", stats.lines);
            }
            3 => {
                let _ = write!(out, "  pieces {}", stats.pieces);
            }
            4 => {
                let _ = write!(out, "  attack {}", stats.attack_sent);
            }
            5 if !engine.pending_garbage().is_empty() => {
                let _ = write!(out, "  incoming {}", engine.pending_garbage().total());
            }
            _ => {}
        }
        let _ = out.write_char('\n');
    }
    let _ = writeln!(out, "  +{}+", "-".repeat(BOARD_W));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Buttons, Handling, MatchConfig, PendingGarbage};

    fn engine() -> Engine {
        Engine::new(1, &MatchConfig::default(), &Handling::default())
    }

    #[test]
    fn a_fresh_board_renders_the_visible_field() {
        let out = render(&engine());
        assert!(out.contains("tick 0"));
        // The frame plus twenty visible rows.
        assert!(out.lines().count() >= VISIBLE_H + 3);
    }

    #[test]
    fn every_row_is_the_width_of_the_board() {
        let out = render(&engine());
        for line in out.lines().filter(|l| l.contains('|')) {
            let inner: String = line
                .chars()
                .skip_while(|c| *c != '|')
                .skip(1)
                .take_while(|c| *c != '|')
                .collect();
            assert_eq!(inner.chars().count(), BOARD_W, "bad row: {line}");
        }
    }

    #[test]
    fn the_active_piece_is_drawn() {
        let out = render(&engine());
        assert!(out.contains('@'), "the falling piece should be visible");
    }

    #[test]
    fn the_landing_position_is_drawn() {
        let out = render(&engine());
        assert!(out.contains(':'), "the landing preview should be visible");
    }

    #[test]
    fn locked_cells_are_drawn() {
        let mut e = engine();
        e.tick(Buttons::HARD_DROP);
        e.tick(Buttons::empty());
        assert!(render(&e).contains('#'), "the stack should be visible");
    }

    #[test]
    fn counters_are_shown() {
        let out = render(&engine());
        for label in ["next", "hold", "lines", "pieces", "attack"] {
            assert!(out.contains(label), "missing {label}:\n{out}");
        }
    }

    #[test]
    fn incoming_garbage_is_shown_only_when_there_is_some() {
        let e = engine();
        assert!(!render(&e).contains("incoming"));

        let mut e = engine();
        e.schedule_garbage(PendingGarbage {
            apply_at_tick: 100_000,
            amount: 4,
            hole_col: 2,
        });
        assert!(render(&e).contains("incoming 4"));
    }

    #[test]
    fn a_tall_stack_reveals_the_buffer_rows() {
        // A stack pushing above the playfield must be visible, or the run that ended the
        // game looks like it ended for no reason.
        let mut e = Engine::new(
            1,
            &MatchConfig {
                garbage_cap: 40,
                ..Default::default()
            },
            &Handling::default(),
        );
        e.schedule_garbage(PendingGarbage {
            apply_at_tick: 1,
            amount: 30,
            hole_col: 0,
        });
        e.tick(Buttons::empty());
        let out = render(&e);
        assert!(out.contains("10|"), "buffer rows should appear:\n{out}");
    }

    #[test]
    fn rendering_never_panics_at_any_point_in_a_game() {
        let mut e = engine();
        for i in 0..2000 {
            e.tick(if i % 5 == 0 {
                Buttons::HARD_DROP
            } else {
                Buttons::empty()
            });
            let _ = render(&e);
        }
    }
}
