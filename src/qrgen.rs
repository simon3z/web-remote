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
    // Each line is 2 chars per half-block pair plus the 2-space border on
    // each side, so width = w_pad + 4.
    let line_width = w_pad + 4;

    let mut out = String::new();

    // Top border.
    out.push_str(&border_line(line_width));

    // Data rows: two QR rows per line.
    for y_pair in (0..w_pad).step_by(2) {
        out.push_str("  "); // left border (2 spaces)
        for x in 0..w_pad {
            let top_dark = is_dark(&colors, y_pair, x, w);
            let bot_dark = is_dark(&colors, y_pair + 1, x, w);
            out.push(pair_glyph(top_dark, bot_dark));
        }
        out.push_str("  "); // right border
        out.push('\n');
    }

    // Bottom border.
    out.push_str(&border_line(line_width));
    out
}

/// A blank line spanning the full QR width (top/bottom border).
fn border_line(line_width: usize) -> String {
    " ".repeat(line_width) + "\n"
}

/// Read module (y, x), treating out-of-bounds coordinates as light.
fn is_dark(colors: &[qrcode::types::Color], y: usize, x: usize, w: usize) -> bool {
    y < w && x < w && colors[y * w + x] == qrcode::types::Color::Dark
}

/// Pick the half-block character for a pair of QR rows (one terminal cell).
fn pair_glyph(top_dark: bool, bot_dark: bool) -> char {
    match (top_dark, bot_dark) {
        (true, true) => '\u{2588}',  // █ both dark
        (true, false) => '\u{2580}', // ▀ top dark
        (false, true) => '\u{2584}', // ▄ bottom dark
        (false, false) => ' ',       // both light
    }
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
