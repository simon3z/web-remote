//! A no-op sink for testing and headless dev: logs the key, emits nothing.
//! Selected with `--input null`. Useful for testing the server/UI without a
//! compositor or uinput device.

use crate::sink::{Key, Sink, SinkError};

pub struct NullSink;

impl NullSink {
    pub fn new() -> Result<Self, SinkError> {
        Ok(Self)
    }
}

impl Sink for NullSink {
    fn emit(&self, key: Key, hold_ms: u32) -> Result<(), SinkError> {
        println!("[null sink] key={:?} hold_ms={}", key, hold_ms);
        Ok(())
    }
}
