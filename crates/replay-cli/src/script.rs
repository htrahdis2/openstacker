//! A text format for writing input by hand.
//!
//! Golden replays have to be authorable without a client, and a JSON array of button
//! bitmasks is not something anyone can write or review. This is the source form; a
//! compiled replay is the artifact.
//!
//! ```text
//! # testdata/scripts/example.script
//! seed: 0x1234
//! mode: sprint40
//!
//! LEFT*12
//! CW
//! .*4          # '.' means no buttons held
//! HARD_DROP
//! garbage: at=1 apply=60 rows=4 hole=3
//! ```
//!
//! The `garbage` line hands rows to the engine the way a server or a sparring opponent
//! would, so a recording can cover rows arriving and being cancelled without a second
//! player in the room.

use engine::{Buttons, PendingGarbage};
use replay::ScheduledGarbage;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub seed: u64,
    pub mode: Option<String>,
    pub buttons: Vec<Buttons>,
    pub garbage: Vec<ScheduledGarbage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ScriptError {}

/// Button names accepted in a script, alongside `.` for nothing held.
const NAMES: &[(&str, Buttons)] = &[
    ("LEFT", Buttons::LEFT),
    ("RIGHT", Buttons::RIGHT),
    ("CW", Buttons::CW),
    ("CCW", Buttons::CCW),
    ("FLIP", Buttons::FLIP),
    ("HOLD", Buttons::HOLD),
    ("SOFT_DROP", Buttons::SOFT_DROP),
    ("HARD_DROP", Buttons::HARD_DROP),
];

pub fn parse(text: &str) -> Result<Script, ScriptError> {
    let mut seed = 0u64;
    let mut mode = None;
    let mut buttons = Vec::new();
    let mut garbage = Vec::new();

    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("seed:") {
            seed = parse_seed(rest.trim()).ok_or_else(|| ScriptError {
                line: line_no,
                message: format!("`{}` is not a number", rest.trim()),
            })?;
            continue;
        }
        if let Some(rest) = line.strip_prefix("mode:") {
            mode = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("garbage:") {
            garbage.push(parse_garbage(rest).map_err(|message| ScriptError {
                line: line_no,
                message,
            })?);
            continue;
        }
        if let Some(rest) = line.strip_prefix("handling:") {
            // Accepted and ignored: handling travels in the replay, and scripts have
            // only ever needed the default. Rejecting it would break older scripts for
            // no benefit.
            let _ = rest;
            continue;
        }

        // A frame: one or more button names joined by `+`, optionally repeated with `*`.
        let (spec, count) = match line.split_once('*') {
            Some((s, n)) => {
                let n: u32 = n.trim().parse().map_err(|_| ScriptError {
                    line: line_no,
                    message: format!("`{}` is not a repeat count", n.trim()),
                })?;
                (s.trim(), n)
            }
            None => (line, 1),
        };

        let frame = parse_frame(spec).map_err(|name| ScriptError {
            line: line_no,
            message: match nearest(&name) {
                Some(s) => format!("unknown button `{name}`, did you mean `{s}`?"),
                None => format!("unknown button `{name}`"),
            },
        })?;
        for _ in 0..count {
            buttons.push(frame);
        }
    }

    Ok(Script {
        seed,
        mode,
        buttons,
        garbage,
    })
}

/// `at=1 apply=60 rows=4 hole=3`, in any order.
///
/// `at` is the tick the rows join the queue and `apply` the tick they enter the board.
/// Both are spelled out because the gap between them is the thing being tested.
fn parse_garbage(rest: &str) -> Result<ScheduledGarbage, String> {
    let (mut at, mut apply, mut rows, mut hole) = (None, None, None, 0u8);
    for part in rest.split_whitespace() {
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| format!("`{part}` should look like `at=1`"))?;
        let n: u32 = value
            .parse()
            .map_err(|_| format!("`{value}` is not a number"))?;
        match key {
            "at" => at = Some(n),
            "apply" => apply = Some(n),
            "rows" => rows = Some(n),
            "hole" => hole = n.min(u8::MAX as u32) as u8,
            other => {
                return Err(format!(
                    "unknown field `{other}`, expected at, apply, rows or hole"
                ));
            }
        }
    }
    let at_tick = at.ok_or("garbage needs `at=`, the tick it is scheduled on")?;
    let apply_at_tick = apply.ok_or("garbage needs `apply=`, the tick it lands on")?;
    let amount = rows.ok_or("garbage needs `rows=`")?;
    if apply_at_tick <= at_tick {
        return Err(format!(
            "rows scheduled on tick {at_tick} cannot land on tick {apply_at_tick}: \
             garbage is always scheduled ahead of where the game is"
        ));
    }
    Ok(ScheduledGarbage {
        at_tick,
        garbage: PendingGarbage {
            apply_at_tick,
            amount: amount.min(u8::MAX as u32) as u8,
            hole_col: hole,
        },
    })
}

fn parse_seed(s: &str) -> Option<u64> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(&hex.replace('_', ""), 16).ok(),
        None => s.replace('_', "").parse().ok(),
    }
}

/// Parse one frame, returning the offending name on failure.
fn parse_frame(spec: &str) -> Result<Buttons, String> {
    if spec == "." {
        return Ok(Buttons::empty());
    }
    let mut out = Buttons::empty();
    for part in spec.split('+') {
        let name = part.trim();
        if name == "." || name.is_empty() {
            continue;
        }
        match NAMES.iter().find(|(n, _)| *n == name) {
            Some((_, b)) => out |= *b,
            None => return Err(name.to_string()),
        }
    }
    Ok(out)
}

fn nearest(name: &str) -> Option<&'static str> {
    NAMES
        .iter()
        .map(|(n, _)| (distance(n, name), *n))
        .min_by_key(|(d, _)| *d)
        .filter(|(d, _)| *d <= 3)
        .map(|(_, n)| n)
}

fn distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let sub = prev[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            cur[j] = sub.min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_script_is_valid() {
        let s = parse("").unwrap();
        assert!(s.buttons.is_empty());
        assert_eq!(s.seed, 0);
    }

    #[test]
    fn headers_are_read() {
        let s = parse("seed: 1234\nmode: sprint40\n").unwrap();
        assert_eq!(s.seed, 1234);
        assert_eq!(s.mode.as_deref(), Some("sprint40"));
    }

    #[test]
    fn garbage_is_read_with_both_of_its_ticks() {
        let s = parse("garbage: at=1 apply=60 rows=4 hole=3\n").unwrap();
        assert_eq!(s.garbage.len(), 1);
        assert_eq!(s.garbage[0].at_tick, 1);
        assert_eq!(s.garbage[0].garbage.apply_at_tick, 60);
        assert_eq!(s.garbage[0].garbage.amount, 4);
        assert_eq!(s.garbage[0].garbage.hole_col, 3);
    }

    #[test]
    fn garbage_fields_may_come_in_any_order_and_the_hole_may_be_left_out() {
        let s = parse("garbage: rows=2 apply=90 at=30\n").unwrap();
        assert_eq!(s.garbage[0].garbage.hole_col, 0);
        assert_eq!(s.garbage[0].at_tick, 30);
    }

    #[test]
    fn garbage_that_lands_before_it_was_scheduled_is_rejected() {
        // The one rule the format enforces, because it is the rule the whole design
        // rests on: rows are always scheduled ahead of where the game is.
        let e = parse("garbage: at=60 apply=30 rows=2\n").unwrap_err();
        assert!(e.message.contains("ahead of where the game is"), "{e}");
    }

    #[test]
    fn an_incomplete_garbage_line_says_what_is_missing() {
        for (text, want) in [
            ("garbage: apply=60 rows=4", "`at=`"),
            ("garbage: at=1 rows=4", "`apply=`"),
            ("garbage: at=1 apply=60", "`rows=`"),
            ("garbage: at=1 apply=60 rows=4 colour=3", "unknown field"),
        ] {
            let e = parse(&format!("{text}\n")).unwrap_err();
            assert!(e.message.contains(want), "{text}: {e}");
        }
    }

    #[test]
    fn a_hex_seed_is_accepted() {
        assert_eq!(parse("seed: 0xFF").unwrap().seed, 255);
        assert_eq!(parse("seed: 0x1234_ABCD").unwrap().seed, 0x1234_ABCD);
    }

    #[test]
    fn single_buttons_become_single_frames() {
        let s = parse("LEFT\nCW\nHARD_DROP\n").unwrap();
        assert_eq!(s.buttons, [Buttons::LEFT, Buttons::CW, Buttons::HARD_DROP]);
    }

    #[test]
    fn a_repeat_count_expands() {
        let s = parse("LEFT*12\n").unwrap();
        assert_eq!(s.buttons.len(), 12);
        assert!(s.buttons.iter().all(|b| *b == Buttons::LEFT));
    }

    #[test]
    fn a_dot_is_a_frame_with_nothing_held() {
        // Needed constantly: an edge-triggered button has to be released before it can
        // fire again, so scripts are half release frames.
        let s = parse(".*3\n").unwrap();
        assert_eq!(s.buttons, [Buttons::empty(); 3]);
    }

    #[test]
    fn buttons_can_be_combined_in_one_frame() {
        let s = parse("LEFT+SOFT_DROP\n").unwrap();
        assert_eq!(s.buttons, [Buttons::LEFT | Buttons::SOFT_DROP]);
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let s = parse("# a comment\n\nLEFT   # trailing\n\n").unwrap();
        assert_eq!(s.buttons, [Buttons::LEFT]);
    }

    #[test]
    fn every_button_has_a_name() {
        // A button with no name could never be written in a script, so a golden replay
        // could never exercise it.
        let mut covered = Buttons::empty();
        for (_, b) in NAMES {
            covered |= *b;
        }
        assert_eq!(covered, Buttons::all());
    }

    #[test]
    fn a_misspelled_button_is_reported_with_a_suggestion() {
        let e = parse("HARDDROP\n").unwrap_err();
        assert_eq!(e.line, 1);
        assert!(e.message.contains("did you mean `HARD_DROP`?"), "{e}");
    }

    #[test]
    fn an_error_names_the_line_it_is_on() {
        let e = parse("LEFT\n.\nNOPE\n").unwrap_err();
        assert_eq!(e.line, 3);
    }

    #[test]
    fn a_bad_repeat_count_is_reported() {
        let e = parse("LEFT*many\n").unwrap_err();
        assert!(e.message.contains("repeat count"), "{e}");
    }

    #[test]
    fn a_realistic_script_parses() {
        let text = "\
# stand an I on end and drop it down the left wall
seed: 0x5EED
mode: sprint40

CW
.
LEFT*12
HARD_DROP
.*2
";
        let s = parse(text).unwrap();
        assert_eq!(s.seed, 0x5EED);
        assert_eq!(s.buttons.len(), 1 + 1 + 12 + 1 + 2);
        assert_eq!(s.buttons[0], Buttons::CW);
        assert_eq!(s.buttons[1], Buttons::empty());
        assert_eq!(s.buttons[14], Buttons::HARD_DROP);
    }
}
