//! web-remote: serve a secure, token-guarded web UI that sends multimedia
//! keys (play/stop/volume) to the local host.
//!
//! The library exposes the pure, host-independent logic (key model, auth,
//! token, host detection, QR) so it can be unit-tested without a compositor
//! or uinput device.

pub mod auth;
pub mod host;
pub mod qrgen;
pub mod sink;

#[cfg(feature = "evdev-sink")]
pub mod privdrop;

#[cfg(feature = "wayland-sink")]
pub mod sink_wayland;

pub mod sink_null;

#[cfg(feature = "evdev-sink")]
pub mod sink_evdev;

pub mod server;
pub mod ui;
