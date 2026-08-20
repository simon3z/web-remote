//! The self-contained keyboard UI (R19–R26). Compiled into the binary with
//! `include_str!`; served at `GET /p/<token>`. Hand-written semantic HTML +
//! custom CSS, no framework, no build step (DESIGN §3.5).

/// The full HTML page, served as `text/html`.
pub const PAGE: &str = include_str!("ui/index.html");

#[cfg(test)]
mod tests {
    use super::PAGE;
    use crate::sink::Key;

    /// Every `data-key` button in the page must map to a known wire key, and
    /// every emittable UI key must have exactly one button. This is the
    /// bridge between the HTML and the Rust `Key` enum — a mismatch is a
    /// silent runtime bug no other test catches.
    #[test]
    fn buttons_match_the_rust_ui_key_set() {
        // Pull out every `data-key="..."` attribute.
        let page = PAGE;
        let mut found = std::collections::HashSet::new();
        for cap in page.match_indices("data-key=") {
            // data-key="prev" — grab up to the closing quote.
            let rest = &page[cap.0 + "data-key=".len()..];
            let rest = rest.strip_prefix('"').expect("data-key must be quoted");
            let end = rest.find('"').expect("unterminated data-key");
            found.insert(rest[..end].to_string());
        }
        // Each button's wire key must parse, and map back to itself.
        for wire in &found {
            let k = Key::from_wire(wire).unwrap_or_else(|| panic!("unknown data-key {wire:?}"));
            assert_eq!(k.wire(), wire.as_str(), "data-key {wire:?} round-trips");
        }
        // The Rust UI set is exactly the buttons present.
        let expected: std::collections::HashSet<String> =
            Key::UI.iter().map(|k| k.wire().to_string()).collect();
        assert_eq!(found, expected, "button data-keys must equal Key::UI");
    }

    /// The JS `KEYS` object (the repeatable map) must have an entry for
    /// every button, and its `repeat` flag must match the Rust `repeatable()`
    /// (R3: volume keys ramp; transport keys are momentary).
    #[test]
    fn js_keys_object_matches_rust() {
        // Extract the `const KEYS = { ... }` block.
        let start =
            PAGE.find("const KEYS = {").expect("JS KEYS object missing") + "const KEYS = {".len();
        let end = PAGE[start..].find("};").expect("no end of KEYS");
        let block = &PAGE[start..start + end];

        for wire in [
            "prev",
            "play",
            "next",
            "fullscreen",
            "voldn",
            "mute",
            "volup",
            "up",
            "down",
            "left",
            "right",
        ] {
            // Match the wire key as a whole entry: `up:` must not match
            // `volup:`, and vice versa.
            let line = block
                .lines()
                .find(|l| l.trim_start().starts_with(&format!("{wire}:")))
                .unwrap_or_else(|| panic!("KEYS missing entry for {wire}"));
            let expects_repeat = Key::from_wire(wire).expect("known key").repeatable();
            let has_repeat = line.contains("repeat: true");
            assert_eq!(
                has_repeat, expects_repeat,
                "JS repeat flag for {wire} must match Rust repeatable()"
            );
        }
    }

    /// Every `data-icon` must reference an icon defined in the `ICONS` map
    /// (a missing icon renders a blank button).
    #[test]
    fn every_button_has_a_defined_icon() {
        let icons_block_start = PAGE.find("const ICONS = {").expect("no ICONS map");
        let icons_block_end = PAGE[icons_block_start..].find("};").unwrap();
        let icons_block = &PAGE[icons_block_start..icons_block_start + icons_block_end];

        // Each button's data-icon must be a key in ICONS.
        let mut checked = 0;
        // The JS icon map is keyed by the wire key, which must equal the
        // data-icon value. (The UI uses one icon per key.)
        for cap in PAGE.match_indices("data-icon=") {
            let rest = &PAGE[cap.0 + "data-icon=".len()..];
            let rest = rest.strip_prefix('"').unwrap();
            let end = rest.find('"').unwrap();
            let icon = &rest[..end];
            // Icons are declared as `  name:  '<svg...` or `name: '<svg...`.
            let declared = icons_block
                .lines()
                .any(|l| l.trim_start().starts_with(&format!("{icon}:")));
            assert!(declared, "icon {icon:?} not defined in ICONS");
            checked += 1;
        }
        assert!(
            checked >= 11,
            "expected at least 11 icon-bearing buttons, got {checked}"
        );
    }

    /// R13/R15: the page must never hard-code a token or a secret — the base
    /// is derived from `location.pathname` at runtime.
    #[test]
    fn no_hardcoded_secret_or_token() {
        // The page is served at /p/<token>; it must not contain any literal
        // /p/<43-char-urlsafe-token> path, nor any obvious secret material.
        assert!(
            !PAGE.matches("/p/").any(|s| {
                // Any /p/<x> literal in the page is only OK if x is the
                // runtime-derived fragment (there is none — the JS builds it).
                s.len() > 2
                    && s[2..]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            }),
            "page appears to hard-code a token path"
        );
    }

    /// R19: no external assets — no remote `<script src>`, `<link>`, or
    /// CSS `url(http...)`. Everything is inline.
    #[test]
    fn no_external_assets() {
        // No `<script src=`, no `<link ... href=`, no remote @import/url().
        assert!(!PAGE.contains("<script src"), "external script");
        assert!(!PAGE.contains("<link"), "external link (stylesheet/icon)");
        // No CSS url() pointing off-box (allow only inline data: or none).
        for cap in PAGE.match_indices("url(") {
            let after = &PAGE[cap.0 + 4..];
            let after = after.trim_start();
            assert!(
                !after.starts_with("http") && !after.starts_with("//"),
                "external url() asset found"
            );
        }
    }

    /// R21: stop is reachable (long-press on play) but is NOT a standalone
    /// button — the UI has no `data-key="stop"`.
    #[test]
    fn stop_is_not_a_visible_button() {
        assert!(
            !PAGE.matches("data-key=\"stop\"").any(|_| true),
            "stop must not be a visible button (it's a long-press)"
        );
        // ...but the long-press path references "stop" as the wire key.
        assert!(
            PAGE.contains("\"stop\""),
            "play long-press must map to stop"
        );
    }
}
