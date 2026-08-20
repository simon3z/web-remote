//! QR code printed to the terminal (R27, R29). Generated locally — no
//! external QR service (N3). Never served over HTTP.

/// Render a QR code as a string of terminal half-blocks.
///
/// Terminal cells are roughly 2:1 (height:width), so we pack two QR rows
/// into one line using the Unicode half-block characters:
///   `▀` = top dark, bottom light
///   `▄` = top light, bottom dark
///   `█` = both dark
///   ` ` = both light
///
/// This makes the rendered QR appear square on screen. A 2-module quiet
/// zone border is included for scannability.
pub fn qr_to_ansi_text(content: &str) -> String {
    let qr = qrcode::QrCode::new(content.as_bytes()).expect("encode QR");
    let w = qr.width();
    let colors = qr.to_colors();

    // Pad to an even width so half-block pairs always align.
    let w_pad = if w.is_multiple_of(2) { w } else { w + 1 };
    // Total lines: (w_pad / 2) data lines + 2 border lines (top/bottom).
    // Each line is 2 chars per half-block pair, so width = w_pad + 4.
    let line_width = w_pad + 4;

    let mut out = String::new();

    // Top border.
    for _ in 0..line_width {
        out.push(' ');
    }
    out.push('\n');

    // Data rows: two QR rows per line.
    for y_pair in (0..w_pad).step_by(2) {
        out.push_str("  "); // left border (2 spaces)
        for x in 0..w_pad {
            let top_dark = if y_pair < w && x < w {
                matches!(colors[y_pair * w + x], qrcode::types::Color::Dark)
            } else {
                false
            };
            let bot_dark = if y_pair + 1 < w && x < w {
                matches!(colors[(y_pair + 1) * w + x], qrcode::types::Color::Dark)
            } else {
                false
            };
            out.push(match (top_dark, bot_dark) {
                (true, true) => '\u{2588}',  // █ both dark
                (true, false) => '\u{2580}', // ▀ top dark
                (false, true) => '\u{2584}', // ▄ bottom dark
                (false, false) => ' ',       // both light
            });
        }
        out.push_str("  "); // right border
        out.push('\n');
    }

    // Bottom border.
    for _ in 0..line_width {
        out.push(' ');
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_renders_nonempty() {
        let text = qr_to_ansi_text("https://192.168.1.5:40443/p/abc");
        assert!(!text.is_empty());
        assert!(text.contains('\n'));
    }

    #[test]
    fn qr_is_deterministic() {
        let a = qr_to_ansi_text("https://example.com/p/tok");
        let b = qr_to_ansi_text("https://example.com/p/tok");
        assert_eq!(a, b);
    }

    /// The QR must actually depend on its content: two different URLs produce
    /// different module patterns (proves we're encoding, not stubbing).
    #[test]
    fn qr_is_content_sensitive() {
        let a = qr_to_ansi_text("https://192.168.1.5:40443/p/TOKEN_A");
        let b = qr_to_ansi_text("https://192.168.1.5:40443/p/TOKEN_B");
        assert_ne!(a, b, "QR must differ for different URLs");
    }

    /// The rendered grid must be square (equal-width lines) so a terminal
    /// renders it undistorted and scanners can read it.
    #[test]
    fn qr_grid_is_square() {
        let text = qr_to_ansi_text("https://192.168.1.5:40443/p/abc");
        let lines: Vec<&str> = text.lines().collect();
        // All non-empty lines have the same char count.
        let widths: std::collections::HashSet<usize> = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.chars().count())
            .collect();
        assert_eq!(
            widths.len(),
            1,
            "all QR lines must be equal width, got: {widths:?}"
        );
    }

    /// The output must use half-block characters (not full blocks) to
    /// compensate for the 2:1 terminal cell aspect ratio.
    #[test]
    fn qr_uses_half_blocks() {
        let text = qr_to_ansi_text("https://192.168.1.5:40443/p/abc");
        // At least one of the four half-block chars must appear.
        assert!(
            text.contains('\u{2588}') || text.contains('\u{2580}') || text.contains('\u{2584}'),
            "expected half-block characters in QR output"
        );
    }
}
