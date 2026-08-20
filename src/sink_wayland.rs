//! The `wayland` sink (R1, R6): no sudo. Emits via `enigo` using the Wayland
//! virtual-keyboard protocol. Default sink.
//!
//! enigo's backend is `!Sync` and owns the (Wayland) connection, so we wrap a
//! single `Enigo` in a `Mutex` and emit sequentially. The server handler holds
//! the lock only for the short duration of one `emit`.

use std::{sync::Mutex, time::Duration};

use enigo::{Direction, Enigo, Key as EnigoKey, Keyboard, Settings};

use crate::sink::{Key, Sink, SinkError, HOLD_CAP_MS};

/// The single, shared Wayland backend (R6: no sudo).
pub struct WaylandSink {
    enigo: Mutex<Enigo>,
}

impl WaylandSink {
    pub fn new() -> Result<Self, SinkError> {
        let settings = Settings::default();
        let enigo = Enigo::new(&settings).map_err(|e| SinkError::Delivery(e.to_string()))?;
        Ok(Self {
            enigo: Mutex::new(enigo),
        })
    }

    fn enigo_key(&self, key: Key) -> EnigoKey {
        match key {
            Key::PlayPause => EnigoKey::MediaPlayPause,
            Key::Stop => EnigoKey::MediaStop,
            Key::Next => EnigoKey::MediaNextTrack,
            Key::Prev => EnigoKey::MediaPrevTrack,
            Key::VolUp => EnigoKey::VolumeUp,
            Key::VolDown => EnigoKey::VolumeDown,
            Key::Mute => EnigoKey::VolumeMute,
            Key::Up => EnigoKey::UpArrow,
            Key::Down => EnigoKey::DownArrow,
            Key::Left => EnigoKey::LeftArrow,
            Key::Right => EnigoKey::RightArrow,
            // F11 is the conventional "toggle fullscreen" key.
            Key::Fullscreen => EnigoKey::F11,
        }
    }
}

impl Sink for WaylandSink {
    fn emit(&self, key: Key, hold_ms: u32) -> Result<(), SinkError> {
        let ek = self.enigo_key(key);
        let mut enigo = self.enigo.lock().expect("enigo mutex poisoned");

        if hold_ms == 0 || !key.repeatable() {
            // Tap: down + up, one event.
            enigo
                .key(ek, Direction::Press)
                .map_err(|e| SinkError::Delivery(e.to_string()))?;
            enigo
                .key(ek, Direction::Release)
                .map_err(|e| SinkError::Delivery(e.to_string()))?;
        } else {
            // Hold for a volume key: hold it down so the kernel auto-repeat
            // ramps the volume, then release. (The virtual-keyboard protocol
            // models a press, not a physical hold; this is a best-effort
            // approximation.)
            enigo
                .key(ek, Direction::Press)
                .map_err(|e| SinkError::Delivery(e.to_string()))?;
            drop(enigo);
            std::thread::sleep(Duration::from_millis(hold_ms.min(HOLD_CAP_MS) as u64));
            let mut enigo = self.enigo.lock().expect("enigo mutex poisoned");
            enigo
                .key(ek, Direction::Release)
                .map_err(|e| SinkError::Delivery(e.to_string()))?;
        }
        Ok(())
    }
}
