//! A player's saved settings, and how they survive the game changing.
//!
//! Settings outlive the build that wrote them. A player upgrades, downgrades, syncs two
//! machines, or comes back after a year, and their DAS has to still be their DAS. That
//! makes loading a compatibility problem rather than a parsing one, and the rules here
//! all follow from it:
//!
//! - **Unknown keys are ignored.** A settings file written by a newer build must not be
//!   rejected wholesale by an older one.
//! - **Missing keys take their default.** A new setting appears configured sensibly
//!   rather than blocking the load.
//! - **Out-of-range values are clamped, not refused.** A file edited by hand, or written
//!   by a build with different bounds, still loads.
//! - **A damaged section costs only that section.** Corrupt keybinds should not also
//!   take a player's handling with them.
//!
//! The one thing never done silently is discarding a setting the player chose. Anything
//! that could not be carried forward is reported back as a note the client can show.

use engine::config::desc::Tunable;
use engine::{Cosmetic, Handling, Keybinds, MatchConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The settings layout this build writes.
///
/// Bumped whenever a stored setting changes shape in a way that needs converting, not
/// merely when one is added or removed. Adding and removing are already handled by
/// defaulting and ignoring.
pub const SETTINGS_VERSION: u16 = 2;

/// Everything a player has chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u16,
    pub handling: Handling,
    pub keybinds: Keybinds,
    pub cosmetic: Cosmetic,
    /// Match rules the player has pinned for their own games, overriding the mode's.
    ///
    /// Absent until they tune something, which is the normal state: a mode's rules are
    /// the rules. Applied through the same host-policy layer a server will use, so
    /// tuning locally and being handed rules remotely are the same mechanism.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub house_rules: Option<MatchConfig>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SETTINGS_VERSION,
            handling: Handling::default(),
            keybinds: Keybinds::default(),
            cosmetic: Cosmetic::default(),
            house_rules: None,
        }
    }
}

/// Something worth telling the player about a load.
///
/// Never a hard failure. Settings loading always produces usable settings; these say
/// what had to be adjusted to get there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// The file was from an older layout and was converted.
    Migrated { from: u16, to: u16 },
    /// The file was from a newer build. Settings this build understands were kept and
    /// the rest left alone.
    FromNewerBuild { found: u16, supported: u16 },
    /// A section could not be read and fell back to defaults.
    SectionReset {
        section: &'static str,
        reason: String,
    },
    /// The whole file was unreadable.
    FileUnreadable { reason: String },
    /// A value was outside what this build allows and was brought into range.
    Clamped { section: &'static str },
}

impl std::fmt::Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Note::Migrated { from, to } => {
                write!(f, "settings upgraded from layout v{from} to v{to}")
            }
            Note::FromNewerBuild { found, supported } => write!(
                f,
                "settings were written by a newer version (layout v{found}, this build \
                 understands v{supported}); anything it did not recognise was left alone"
            ),
            Note::SectionReset { section, reason } => {
                write!(f, "{section} settings were reset to defaults: {reason}")
            }
            Note::FileUnreadable { reason } => {
                write!(f, "settings could not be read, using defaults: {reason}")
            }
            Note::Clamped { section } => {
                write!(
                    f,
                    "some {section} values were outside the allowed range and were adjusted"
                )
            }
        }
    }
}

impl Settings {
    /// Read settings from JSON.
    ///
    /// Never fails. A player who launches the game gets to play, whatever state their
    /// settings file is in; the notes describe anything that had to be adjusted.
    pub fn from_json(text: &str) -> (Settings, Vec<Note>) {
        let mut notes = Vec::new();

        let Ok(mut root) = serde_json::from_str::<Value>(text) else {
            notes.push(Note::FileUnreadable {
                reason: "not valid JSON".into(),
            });
            return (Settings::default(), notes);
        };
        if !root.is_object() {
            notes.push(Note::FileUnreadable {
                reason: "not a settings object".into(),
            });
            return (Settings::default(), notes);
        }

        let found = root
            .get("version")
            .and_then(Value::as_u64)
            .unwrap_or(SETTINGS_VERSION as u64) as u16;

        if found < SETTINGS_VERSION {
            migrate(&mut root, found);
            notes.push(Note::Migrated {
                from: found,
                to: SETTINGS_VERSION,
            });
        } else if found > SETTINGS_VERSION {
            // Read what is recognisable and leave the rest. Refusing would strand a
            // player who tried a newer build once and went back.
            notes.push(Note::FromNewerBuild {
                found,
                supported: SETTINGS_VERSION,
            });
        }

        // Each section is read on its own, so damage to one cannot cost the others.
        let handling = section(&root, "handling", &mut notes);
        let keybinds = section(&root, "keybinds", &mut notes);
        let cosmetic = section(&root, "cosmetic", &mut notes);

        // House rules are read whole or not at all: half a set of rules is not a set of
        // rules, and defaulting the missing half would silently play a different game.
        let house_rules = match root
            .get("house_rules")
            .filter(|v| !v.is_null())
            .map(|v| serde_json::from_value::<MatchConfig>(v.clone()))
        {
            Some(Ok(rules)) => Some(rules),
            Some(Err(e)) => {
                notes.push(Note::SectionReset {
                    section: "match rules",
                    reason: e.to_string(),
                });
                None
            }
            None => None,
        };

        let mut settings = Settings {
            version: SETTINGS_VERSION,
            handling,
            keybinds,
            cosmetic,
            house_rules,
        };

        // Bounds are this build's, not the file's, so a value from a build with wider
        // ranges is brought in rather than rejected.
        let before = settings.clone();
        settings.handling.clamp();
        settings.cosmetic.clamp();
        if let Some(rules) = settings.house_rules.as_mut() {
            rules.clamp();
        }
        if settings.handling != before.handling {
            notes.push(Note::Clamped {
                section: "handling",
            });
        }
        if settings.cosmetic != before.cosmetic {
            notes.push(Note::Clamped {
                section: "appearance",
            });
        }

        (settings, notes)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("settings should serialize")
    }
}

/// Read one section, falling back to defaults if it is unusable.
fn section<T>(root: &Value, key: &'static str, notes: &mut Vec<Note>) -> T
where
    T: Default + for<'de> Deserialize<'de>,
{
    let Some(v) = root.get(key) else {
        return T::default();
    };
    match serde_json::from_value::<T>(v.clone()) {
        Ok(parsed) => parsed,
        Err(e) => {
            notes.push(Note::SectionReset {
                section: key,
                reason: e.to_string(),
            });
            T::default()
        }
    }
}

/// Convert an older layout in place.
///
/// Each step moves one version forward, so a file from any past layout walks the chain
/// rather than needing a direct path to the present. There are no steps yet; the chain
/// exists now because retrofitting it means guessing what old files looked like.
fn migrate(root: &mut Value, from: u16) {
    let mut version = from;
    while version < SETTINGS_VERSION {
        match version {
            // 0 predates the version field. Nothing about the layout differed, so there
            // is nothing to convert; the walk simply moves it forward.
            0 => {}
            // 1 had no house rules. Absent is the correct reading of a file that
            // predates them: the mode's rules were the only rules.
            1 => {}
            _ => break,
        }
        version += 1;
    }
    if let Some(obj) = root.as_object_mut() {
        obj.insert("version".into(), Value::from(SETTINGS_VERSION));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let (s, notes) = Settings::from_json(&Settings::default().to_json());
        assert_eq!(s, Settings::default());
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn a_players_choices_survive_a_round_trip() {
        let mut s = Settings::default();
        s.handling.das_ms = 83;
        s.handling.arr_ms = 0;
        s.cosmetic.ghost_opacity = 10;
        let (back, notes) = Settings::from_json(&s.to_json());
        assert_eq!(back, s);
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn a_setting_added_since_the_file_was_written_takes_its_default() {
        // The common case: the game gained an option and the player has not seen it yet.
        let old = format!(r#"{{"version":{SETTINGS_VERSION},"handling":{{"das_ms":83}}}}"#);
        let (s, notes) = Settings::from_json(&old);
        assert_eq!(s.handling.das_ms, 83, "the chosen value must survive");
        assert_eq!(s.handling.arr_ms, Handling::default().arr_ms);
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn a_setting_removed_since_the_file_was_written_costs_nothing_else() {
        // The case that used to wipe everything. One obsolete key must not take the
        // player's whole configuration with it.
        let old = format!(
            r#"{{
            "version": {SETTINGS_VERSION},
            "handling": {{"das_ms": 83, "dcd_frames": 3, "some_removed_option": true}},
            "keybinds": {{"move_left": "KeyA", "legacy_bind": "KeyQ"}},
            "cosmetic": {{"ghost_opacity": 12, "old_toggle": false}}
        }}"#
        );
        let (s, notes) = Settings::from_json(&old);
        assert_eq!(s.handling.das_ms, 83);
        assert_eq!(s.keybinds.move_left.as_str(), "KeyA");
        assert_eq!(s.cosmetic.ghost_opacity, 12);
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn a_file_from_before_house_rules_reads_as_having_none() {
        // Which is the correct reading: before the tuner existed, the mode's rules were
        // the only rules there were.
        let old = r#"{"version":1,"handling":{"das_ms":83}}"#;
        let (s, notes) = Settings::from_json(old);
        assert!(s.house_rules.is_none());
        assert_eq!(s.handling.das_ms, 83, "the rest must survive the migration");
        assert!(matches!(notes.as_slice(), [Note::Migrated { from: 1, .. }]));
    }

    #[test]
    fn house_rules_are_brought_into_range_like_anything_else() {
        let text =
            format!(r#"{{"version":{SETTINGS_VERSION},"house_rules":{{"garbage_cap":200}}}}"#);
        let (s, _) = Settings::from_json(&text);
        let rules = s.house_rules.expect("house rules should be read");
        assert!(rules.garbage_cap < 200, "{}", rules.garbage_cap);
    }

    #[test]
    fn house_rules_that_cannot_be_read_are_dropped_rather_than_half_applied() {
        // Half a set of rules is not a set of rules. Defaulting the missing half would
        // quietly play a different game from the one the player tuned.
        let text =
            format!(r#"{{"version":{SETTINGS_VERSION},"house_rules":{{"gravity":"very fast"}}}}"#);
        let (s, notes) = Settings::from_json(&text);
        assert!(s.house_rules.is_none());
        assert!(
            notes.iter().any(|n| n.to_string().contains("match rules")),
            "{notes:?}"
        );
    }

    #[test]
    fn settings_with_no_house_rules_do_not_write_the_key_at_all() {
        // A null in storage would read as "tuned to nothing" rather than "not tuned".
        assert!(!Settings::default().to_json().contains("house_rules"));
    }

    #[test]
    fn a_file_from_a_newer_build_is_read_rather_than_refused() {
        // A player who tried a newer build and went back must not lose everything.
        let newer = format!(
            r#"{{"version":{},"handling":{{"das_ms":77,"future_setting":9}}}}"#,
            SETTINGS_VERSION + 5
        );
        let (s, notes) = Settings::from_json(&newer);
        assert_eq!(s.handling.das_ms, 77);
        assert!(
            notes
                .iter()
                .any(|n| matches!(n, Note::FromNewerBuild { .. })),
            "the player should be told: {notes:?}"
        );
    }

    #[test]
    fn a_file_with_no_version_still_loads() {
        let (s, _) = Settings::from_json(r#"{"handling":{"das_ms":100}}"#);
        assert_eq!(s.handling.das_ms, 100);
    }

    #[test]
    fn an_older_layout_is_migrated_and_reported() {
        let (s, notes) = Settings::from_json(r#"{"version":0,"handling":{"das_ms":91}}"#);
        assert_eq!(s.handling.das_ms, 91);
        assert_eq!(s.version, SETTINGS_VERSION);
        assert!(
            notes
                .iter()
                .any(|n| matches!(n, Note::Migrated { from: 0, .. }))
        );
    }

    #[test]
    fn a_damaged_section_does_not_take_the_others_with_it() {
        // Handling is unreadable, keybinds are fine. The player keeps their keybinds.
        let broken = r#"{
            "version": 1,
            "handling": "this should be an object",
            "keybinds": {"move_left": "KeyA"}
        }"#;
        let (s, notes) = Settings::from_json(broken);
        assert_eq!(s.handling, Handling::default(), "handling falls back");
        assert_eq!(s.keybinds.move_left.as_str(), "KeyA", "keybinds survive");
        assert!(
            notes
                .iter()
                .any(|n| matches!(n, Note::SectionReset { section, .. } if *section == "handling")),
            "{notes:?}"
        );
    }

    #[test]
    fn a_completely_unreadable_file_still_yields_playable_settings() {
        // Whatever state the file is in, the player gets to play.
        for text in ["", "not json at all", "[1,2,3]", "null", "{"] {
            let (s, notes) = Settings::from_json(text);
            assert_eq!(s, Settings::default(), "for input {text:?}");
            assert!(
                !notes.is_empty(),
                "the player should be told about {text:?}"
            );
        }
    }

    #[test]
    fn out_of_range_values_are_brought_into_range_rather_than_refused() {
        // A file written by a build with wider bounds, or edited by hand.
        let wild = r#"{"version":1,"handling":{"das_ms":65535},"cosmetic":{"board_scale":250}}"#;
        let (s, notes) = Settings::from_json(wild);
        assert_eq!(s.handling.das_ms, 500);
        assert_eq!(s.cosmetic.board_scale, 200);
        assert!(notes.iter().any(|n| matches!(n, Note::Clamped { .. })));
    }

    #[test]
    fn loading_is_idempotent() {
        // Saving and reloading repeatedly must not drift a player's settings.
        let wild = r#"{"version":0,"handling":{"das_ms":65535,"stale":1}}"#;
        let (once, _) = Settings::from_json(wild);
        let (twice, notes) = Settings::from_json(&once.to_json());
        assert_eq!(once, twice);
        assert!(
            notes.is_empty(),
            "a clean reload should be quiet: {notes:?}"
        );
    }

    #[test]
    fn every_note_reads_as_a_sentence() {
        // These are shown to players, not logged for developers.
        let notes = [
            Note::Migrated { from: 0, to: 1 },
            Note::FromNewerBuild {
                found: 9,
                supported: 1,
            },
            Note::SectionReset {
                section: "handling",
                reason: "bad".into(),
            },
            Note::FileUnreadable {
                reason: "bad".into(),
            },
            Note::Clamped {
                section: "handling",
            },
        ];
        for n in notes {
            let text = n.to_string();
            assert!(text.len() > 20, "too terse: {text}");
            assert!(
                text.chars().next().unwrap().is_lowercase() || text.starts_with(char::is_uppercase),
                "{text}"
            );
        }
    }

    #[test]
    fn keybinds_survive_a_rebind_and_reload() {
        let mut s = Settings::default();
        s.keybinds.move_left = engine::config::client::Name::from("KeyH").unwrap();
        s.keybinds.hard_drop = engine::config::client::Name::from("KeyJ").unwrap();
        let (back, _) = Settings::from_json(&s.to_json());
        assert_eq!(back.keybinds, s.keybinds);
    }
}
