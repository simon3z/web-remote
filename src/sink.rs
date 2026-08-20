//! The keys the tool can emit, and the pluggable sink abstraction.
//!
//! R1–R4: only these keys exist. There is no code path to a generic typing
//! key — the wire and the API both speak this closed `Key` enum.

/// The only keys the tool can emit. The wire format (`"volup"`, `"play"`, …)
/// and the API allow-list are both defined over this enum, so the set of
/// emittable keys is closed and auditable (R4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    PlayPause,
    Stop,
    Next,
    Prev,
    VolUp,
    VolDown,
    Mute,
    Up,
    Down,
    Left,
    Right,
    Fullscreen,
}

impl Key {
    /// Parse from the wire token. `None` if not one of the emittable keys.
    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            "play" => Some(Self::PlayPause),
            "stop" => Some(Self::Stop),
            "next" => Some(Self::Next),
            "prev" => Some(Self::Prev),
            "volup" => Some(Self::VolUp),
            "voldn" => Some(Self::VolDown),
            "mute" => Some(Self::Mute),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "fullscreen" => Some(Self::Fullscreen),
            _ => None,
        }
    }

    /// The wire token for this key (used by the UI, the API, and tests).
    pub fn wire(self) -> &'static str {
        match self {
            Self::PlayPause => "play",
            Self::Stop => "stop",
            Self::Next => "next",
            Self::Prev => "prev",
            Self::VolUp => "volup",
            Self::VolDown => "voldn",
            Self::Mute => "mute",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::Fullscreen => "fullscreen",
        }
    }

    /// Every emittable key, in UI/transport order. Used to build the UI and
    /// to assert the closed set.
    /// The eleven UI keys, in grid order (R20). `Stop` is not a UI button — it's
    /// reached by long-press on play/pause (R21) — but is emittable.
    pub const UI: [Key; 11] = [
        Key::Prev,
        Key::PlayPause,
        Key::Next,
        Key::VolDown,
        Key::Mute,
        Key::VolUp,
        Key::Fullscreen,
        Key::Up,
        Key::Left,
        Key::Down,
        Key::Right,
    ];

    /// Whether this key can be held (auto-repeat / volume ramp) for `hold_ms`.
    /// Volume keys ramp with kernel auto-repeat; transport keys are
    /// momentary (a hold is a no-op beyond a single event).
    pub fn repeatable(self) -> bool {
        matches!(self, Key::VolUp | Key::VolDown)
    }
}

/// The server-side cap on a requested hold, in milliseconds (R15).
pub const HOLD_CAP_MS: u32 = 5000;

/// The pluggable input backend (R1). Three implementations: `evdev` (root,
/// any compositor) and `wayland` (no root, GNOME). Both receive the same
/// `Key` and `hold_ms` and emit to the host.
///
/// `hold_ms` is the *request* duration; the backend decides how to honor it
/// (evdev: real down/delay/up; wayland: a single momentary event per the
/// virtual-keyboard protocol). The kernel auto-repeat handles volume ramping
/// on the evdev path.
pub trait Sink: Send + Sync {
    /// Emit `key`, held down for `hold_ms` milliseconds (0 = tap).
    fn emit(&self, key: Key, hold_ms: u32) -> Result<(), SinkError>;
}

/// Errors a sink can return. Mapped to a 500 by the server (the token is
/// valid, but the host couldn't deliver the key).
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    #[error("key delivery failed: {0}")]
    Delivery(String),
}

#[cfg(test)]
mod tests {
    use super::Key;

    /// R4: the emittable set is exactly these 12 wire tokens. Every one must
    /// round-trip through `from_wire`/`wire`, and nothing else may parse.
    #[test]
    fn wire_round_trip_is_exactly_twelve() {
        let all = [
            Key::PlayPause,
            Key::Stop,
            Key::Next,
            Key::Prev,
            Key::VolUp,
            Key::VolDown,
            Key::Mute,
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Fullscreen,
        ];
        for k in all {
            assert_eq!(Key::from_wire(k.wire()), Some(k), "round-trip {k:?}");
        }
    }

    #[test]
    fn from_wire_rejects_unknown() {
        // Unknown, empty, and the generic keys we deliberately exclude (R4).
        for bad in [
            "",
            "a",
            "enter",
            "space",
            "tab",
            "KEY_PLAYPAUSE",
            "PLAY",
            "playPause",
            "volUP",
            "f11",
            "F11",
        ] {
            assert_eq!(Key::from_wire(bad), None, "must reject {bad:?}");
        }
    }

    #[test]
    fn stop_is_emittable_but_not_a_ui_button() {
        // R21: stop exists (long-press) but is not in the UI grid.
        assert_eq!(Key::from_wire("stop"), Some(Key::Stop));
        assert!(
            !Key::UI.contains(&Key::Stop),
            "stop must not be a UI button"
        );
        assert_eq!(Key::UI.len(), 11);
    }

    #[test]
    fn only_volume_keys_are_repeatable() {
        assert!(Key::VolUp.repeatable());
        assert!(Key::VolDown.repeatable());
        for k in [
            Key::PlayPause,
            Key::Stop,
            Key::Next,
            Key::Prev,
            Key::Mute,
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Fullscreen,
        ] {
            assert!(!k.repeatable(), "{k:?} should not be repeatable");
        }
    }
}
