//! Combining config from several sources, and remembering which one won.
//!
//! Settings arrive from four places, each allowed to override the one before it:
//!
//! ```text
//! defaults  <-  mode file  <-  host policy  <-  player
//! ```
//!
//! Tracking *which* layer supplied each value is what lets a settings screen grey out a
//! control and say why, instead of accepting an edit that silently does nothing. A
//! player who drags a slider that a mode has already fixed, and sees no effect, will
//! reasonably conclude the game is broken.

use engine::config::desc::Tunable;
use engine::{Handling, MatchConfig};

/// Where a resolved value came from, in increasing order of precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// Built into the engine.
    Default,
    /// Set by the mode being played.
    Mode,
    /// Set by whoever is hosting, which may cap or pin a setting.
    HostPolicy,
    /// Chosen by the player.
    Player,
}

impl Layer {
    /// Human-readable reason a control is locked, for a settings UI.
    pub const fn locked_reason(self) -> &'static str {
        match self {
            Layer::Default => "engine default",
            Layer::Mode => "fixed by this mode",
            Layer::HostPolicy => "fixed by the host",
            Layer::Player => "your setting",
        }
    }
}

/// What a host may impose on top of a mode.
///
/// Empty by default, and stays empty for a local game. It exists now so that adding
/// remote play later does not change the shape of resolution.
#[derive(Debug, Clone, Default)]
pub struct HostPolicy {
    /// Rules the host pins, overriding the mode.
    pub match_config: Option<MatchConfig>,
    /// Handling the host pins, overriding the player. Rare, and mostly for events that
    /// want everyone on identical settings.
    pub handling: Option<Handling>,
}

/// The result of resolution: the values to play with, and where each group came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub match_config: MatchConfig,
    pub match_config_layer: Layer,
    pub handling: Handling,
    pub handling_layer: Layer,
}

impl Resolved {
    /// Whether the player is allowed to change their own handling.
    pub fn handling_is_locked(&self) -> bool {
        self.handling_layer == Layer::HostPolicy
    }

    /// Whether the match rules are fixed by something above the player.
    pub fn match_config_is_locked(&self) -> bool {
        self.match_config_layer >= Layer::Mode
    }
}

/// Apply the layers in precedence order.
///
/// Player handling is clamped, because it arrives from an untrusted client and clamping
/// is deterministic: every peer lands on the same values rather than one rejecting what
/// another accepts. Mode config is *not* clamped here, because a file is authored and
/// the loader has already rejected anything out of range with a message naming the line.
pub fn resolve(
    mode: Option<&MatchConfig>,
    policy: &HostPolicy,
    player: Option<&Handling>,
) -> Resolved {
    let (match_config, match_config_layer) = match (&policy.match_config, mode) {
        (Some(p), _) => (p.clone(), Layer::HostPolicy),
        (None, Some(m)) => (m.clone(), Layer::Mode),
        (None, None) => (MatchConfig::default(), Layer::Default),
    };

    let (mut handling, handling_layer) = match (&policy.handling, player) {
        (Some(p), _) => (*p, Layer::HostPolicy),
        (None, Some(p)) => (*p, Layer::Player),
        (None, None) => (Handling::default(), Layer::Default),
    };
    handling.clamp();

    Resolved {
        match_config,
        match_config_layer,
        handling,
        handling_layer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode_config() -> MatchConfig {
        MatchConfig {
            lock_delay_ms: 250,
            ..Default::default()
        }
    }

    #[test]
    fn nothing_supplied_gives_engine_defaults() {
        let r = resolve(None, &HostPolicy::default(), None);
        assert_eq!(r.match_config, MatchConfig::default());
        assert_eq!(r.handling, Handling::default());
        assert_eq!(r.match_config_layer, Layer::Default);
        assert_eq!(r.handling_layer, Layer::Default);
    }

    #[test]
    fn a_mode_overrides_the_defaults() {
        let r = resolve(Some(&mode_config()), &HostPolicy::default(), None);
        assert_eq!(r.match_config.lock_delay_ms, 250);
        assert_eq!(r.match_config_layer, Layer::Mode);
    }

    #[test]
    fn a_host_overrides_the_mode() {
        let policy = HostPolicy {
            match_config: Some(MatchConfig {
                lock_delay_ms: 100,
                ..Default::default()
            }),
            handling: None,
        };
        let r = resolve(Some(&mode_config()), &policy, None);
        assert_eq!(r.match_config.lock_delay_ms, 100);
        assert_eq!(r.match_config_layer, Layer::HostPolicy);
        assert!(r.match_config_is_locked());
    }

    #[test]
    fn a_player_supplies_their_own_handling() {
        let h = Handling {
            das_ms: 90,
            ..Default::default()
        };
        let r = resolve(None, &HostPolicy::default(), Some(&h));
        assert_eq!(r.handling.das_ms, 90);
        assert_eq!(r.handling_layer, Layer::Player);
        assert!(!r.handling_is_locked());
    }

    #[test]
    fn a_host_can_pin_handling_over_the_player() {
        let policy = HostPolicy {
            match_config: None,
            handling: Some(Handling {
                das_ms: 133,
                ..Default::default()
            }),
        };
        let player = Handling {
            das_ms: 0,
            ..Default::default()
        };
        let r = resolve(None, &policy, Some(&player));
        assert_eq!(r.handling.das_ms, 133);
        assert_eq!(r.handling_layer, Layer::HostPolicy);
        assert!(
            r.handling_is_locked(),
            "the settings screen has to be able to say why the slider does nothing"
        );
    }

    #[test]
    fn player_handling_is_clamped_during_resolution() {
        // Straight off the wire, so it cannot be trusted. Every peer must clamp to the
        // same values rather than one accepting what another rejects.
        let wild = Handling {
            das_ms: u16::MAX,
            arr_ms: u16::MAX,
            ..Default::default()
        };
        let r = resolve(None, &HostPolicy::default(), Some(&wild));
        assert_eq!(r.handling.das_ms, 500);
        assert_eq!(r.handling.arr_ms, 200);
    }

    #[test]
    fn host_pinned_handling_is_clamped_too() {
        // A host is not automatically trustworthy either, and an unclamped pinned value
        // would put every client in that room outside the range the engine expects.
        let policy = HostPolicy {
            match_config: None,
            handling: Some(Handling {
                das_ms: u16::MAX,
                ..Default::default()
            }),
        };
        let r = resolve(None, &policy, None);
        assert_eq!(r.handling.das_ms, 500);
    }

    #[test]
    fn resolution_is_deterministic_for_the_same_inputs() {
        // Two peers resolving the same layers must reach identical values, or they
        // diverge before the first piece falls.
        let h = Handling {
            das_ms: 9999,
            ..Default::default()
        };
        let a = resolve(Some(&mode_config()), &HostPolicy::default(), Some(&h));
        let b = resolve(Some(&mode_config()), &HostPolicy::default(), Some(&h));
        assert_eq!(a, b);
    }

    #[test]
    fn layers_order_by_precedence() {
        assert!(Layer::Default < Layer::Mode);
        assert!(Layer::Mode < Layer::HostPolicy);
        assert!(Layer::HostPolicy < Layer::Player);
    }

    #[test]
    fn every_layer_has_a_reason_string_for_the_ui() {
        for l in [
            Layer::Default,
            Layer::Mode,
            Layer::HostPolicy,
            Layer::Player,
        ] {
            assert!(!l.locked_reason().is_empty(), "{l:?}");
        }
    }
}
