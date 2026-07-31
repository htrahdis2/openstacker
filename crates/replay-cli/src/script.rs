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
//! ```

use engine::Buttons;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub seed: u64,
    pub mode: Option<String>,
    pub buttons: Vec<Buttons>,
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
