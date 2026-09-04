/// Wrap `text` with ANSI SGR sequence `\x1b[<code>m<text>\x1b[0m`.
pub fn s(code: u8, text: &str) -> String {
    format!("\x1b[{code}m{text}\x1b[0m")
}

pub const CHECK: &str = "✓";
pub const CIRCLE: &str = "●";
pub const MINUS: &str = "−"; // Unicode MINUS SIGN \u{2212}
pub const PLUS: &str = "+";
pub const TILDE: &str = "~";
pub const WARNING: &str = "⚠";
pub const CROSS: &str = "✗";
