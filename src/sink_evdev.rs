//! The `evdev` sink (R1, R7): works on any compositor; needs a short root
//! window to open `/dev/uinput` and register a virtual keyboard, then drops
//! privileges (handled by `privdrop`). Only the emittable keycodes are
//! registered (R4).

use std::{sync::Mutex, time::Duration};

use evdev::{uinput::VirtualDevice, AttributeSet, InputEvent, KeyCode, KeyEvent};

use crate::sink::{Key, Sink, SinkError, HOLD_CAP_MS};

/// The single, shared evdev backend. The `VirtualDevice` is wrapped in a
/// `Mutex` so the sink is `Send + Sync` (the `Sink` bound) and concurrent
/// requests serialize on the one fd.
pub struct EvdevSink {
    device: Mutex<VirtualDevice>,
}

/// The keycodes the virtual keyboard advertises. Only these (R4) — no
/// generic typing keys are registered.
fn media_keycodes() -> AttributeSet<KeyCode> {
    let mut s = AttributeSet::new();
    s.insert(KeyCode::KEY_PLAYPAUSE);
    s.insert(KeyCode::KEY_STOP);
    s.insert(KeyCode::KEY_NEXTSONG);
    s.insert(KeyCode::KEY_PREVIOUSSONG);
    s.insert(KeyCode::KEY_VOLUMEUP);
    s.insert(KeyCode::KEY_VOLUMEDOWN);
    s.insert(KeyCode::KEY_MUTE);
    s.insert(KeyCode::KEY_UP);
    s.insert(KeyCode::KEY_DOWN);
    s.insert(KeyCode::KEY_LEFT);
    s.insert(KeyCode::KEY_RIGHT);
    s.insert(KeyCode::KEY_F11);
    s
}

impl EvdevSink {
    pub fn new() -> Result<Self, SinkError> {
        let device = evdev::uinput::VirtualDevice::builder()
            .map_err(|e| SinkError::Delivery(e.to_string()))?
            .name("web-remote media")
            .with_keys(&media_keycodes())
            .map_err(|e| SinkError::Delivery(e.to_string()))?
            .build()
            .map_err(|e| SinkError::Delivery(e.to_string()))?;
        Ok(Self {
            device: Mutex::new(device),
        })
    }

    fn keycode(&self, key: Key) -> KeyCode {
        match key {
            Key::PlayPause => KeyCode::KEY_PLAYPAUSE,
            Key::Stop => KeyCode::KEY_STOP,
            Key::Next => KeyCode::KEY_NEXTSONG,
            Key::Prev => KeyCode::KEY_PREVIOUSSONG,
            Key::VolUp => KeyCode::KEY_VOLUMEUP,
            Key::VolDown => KeyCode::KEY_VOLUMEDOWN,
            Key::Mute => KeyCode::KEY_MUTE,
            Key::Up => KeyCode::KEY_UP,
            Key::Down => KeyCode::KEY_DOWN,
            Key::Left => KeyCode::KEY_LEFT,
            Key::Right => KeyCode::KEY_RIGHT,
            Key::Fullscreen => KeyCode::KEY_F11,
        }
    }
}

impl Sink for EvdevSink {
    fn emit(&self, key: Key, hold_ms: u32) -> Result<(), SinkError> {
        let mut device = self.device.lock().expect("evdev mutex poisoned");
        let code = self.keycode(key);
        let down = InputEvent::from(KeyEvent::new(code, 1));
        let up = InputEvent::from(KeyEvent::new(code, 0));
        let cap = hold_ms.min(HOLD_CAP_MS);
        if cap == 0 {
            device
                .emit(&[down, up])
                .map_err(|e| SinkError::Delivery(e.to_string()))?;
        } else {
            device
                .emit(&[down])
                .map_err(|e| SinkError::Delivery(e.to_string()))?;
            // Hold while the lock is NOT held (release before sleeping so
            // other requests aren't serialized behind a long sleep).
            drop(device);
            std::thread::sleep(Duration::from_millis(cap as u64));
            let mut device = self.device.lock().expect("evdev mutex poisoned");
            device
                .emit(&[up])
                .map_err(|e| SinkError::Delivery(e.to_string()))?;
        }
        Ok(())
    }
}
