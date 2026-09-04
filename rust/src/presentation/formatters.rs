use crate::core::version::Version;
use crate::fs::scanner::FileStatus;
use crate::presentation::ansi::{s, CHECK, CIRCLE, CROSS, MINUS, PLUS, TILDE, WARNING};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationMode {
    Plain,
    Terminal,
}

impl PresentationMode {
    pub fn is_terminal(&self) -> bool {
        *self == PresentationMode::Terminal
    }
}

/// Format successful lifecycle action output (`init`, `commit`, `revert`, `merge`).
pub fn format_action_success(label: &str, version: &Version, mode: PresentationMode) -> String {
    if mode.is_terminal() {
        format!(
            "{} {} {}\n",
            s(32, CHECK),
            s(1, label),
            s(36, &version.to_string())
        )
    } else {
        format!("{version}\n")
    }
}

/// File status entry for status formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRow {
    pub path: String,
    pub status: FileStatus,
}

/// Format `snap status` output.
pub fn format_status(version: &Version, entries: &[StatusRow], mode: PresentationMode) -> String {
    if mode.is_terminal() {
        let mut out = format!(
            "{}  {}\n\n",
            s(1, "Snap status"),
            s(36, &version.to_string())
        );
        if entries.is_empty() {
            out.push_str(&format!("  {} Working tree clean\n", s(32, CHECK)));
        } else {
            for entry in entries {
                let (color, sym, label) = match entry.status {
                    FileStatus::Added => (32, PLUS, "added"),
                    FileStatus::Deleted => (31, MINUS, "deleted"),
                    FileStatus::Modified => (33, TILDE, "modified"),
                };
                out.push_str(&format!(
                    "  {} {} {}\n",
                    s(color, sym),
                    entry.path,
                    s(2, &format!("({label})"))
                ));
            }
        }
        out
    } else {
        let mut out = format!("version {version}\n");
        for entry in entries {
            let code = match entry.status {
                FileStatus::Added => 'A',
                FileStatus::Deleted => 'D',
                FileStatus::Modified => 'M',
            };
            out.push_str(&format!("{code} {}\n", entry.path));
        }
        out
    }
}

/// Log record for presentation formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    pub version: Version,
    pub author: String,
    pub escaped_message: String,
}

/// Format `snap log` output.
pub fn format_log(entries: &[LogRecord], mode: PresentationMode) -> String {
    if entries.is_empty() {
        return String::new();
    }

    if mode.is_terminal() {
        let blocks: Vec<String> = entries
            .iter()
            .map(|e| {
                format!(
                    "{} {}\n  {} {} {}\n",
                    s(36, CIRCLE),
                    s(1, &e.escaped_message),
                    s(36, &e.version.to_string()),
                    s(2, "by"),
                    s(35, &e.author)
                )
            })
            .collect();
        blocks.join("\n")
    } else {
        let mut out = String::new();
        for e in entries {
            out.push_str(&format!(
                "{}\t{}\t{}\n",
                e.version, e.author, e.escaped_message
            ));
        }
        out
    }
}

/// Format unified or binary diff output.
pub fn format_diff(plain_diff: &str, mode: PresentationMode) -> String {
    if !mode.is_terminal() || plain_diff.is_empty() {
        return plain_diff.to_string();
    }

    let mut out = String::new();
    for line in plain_diff.lines() {
        let styled = if line.starts_with("--- ") || line.starts_with("+++ ") {
            s(1, line)
        } else if line.starts_with("@@ ") {
            s(36, line)
        } else if line.starts_with('-') {
            s(31, line)
        } else if line.starts_with('+') {
            s(32, line)
        } else if line.starts_with("\\ ") {
            s(2, line)
        } else if line.starts_with("Binary files ") {
            s(33, line)
        } else {
            line.to_string()
        };
        out.push_str(&styled);
        out.push('\n');
    }
    out
}

/// Format `snap --version` output.
pub fn format_version(semver: &str, mode: PresentationMode) -> String {
    let plain = format!("snap {semver}");
    if mode.is_terminal() {
        format!("{}\n", s(1, &plain))
    } else {
        format!("{plain}\n")
    }
}

/// Format warning line output.
pub fn format_warning(detail: &str, mode: PresentationMode) -> String {
    if mode.is_terminal() {
        format!("{} {}\n", s(33, WARNING), s(33, detail))
    } else {
        format!("warning: {detail}\n")
    }
}

/// Format error line output.
pub fn format_error(error_msg: &str, mode: PresentationMode) -> String {
    if mode.is_terminal() {
        format!("{}\n", s(31, &format!("{CROSS} {error_msg}")))
    } else {
        format!("{error_msg}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_action_success() {
        let v = Version::empty();
        let term = format_action_success("Initialized repository", &v, PresentationMode::Terminal);
        assert_eq!(
            term,
            "\x1b[32m✓\x1b[0m \x1b[1mInitialized repository\x1b[0m \x1b[36m()\x1b[0m\n"
        );

        let plain = format_action_success("Initialized repository", &v, PresentationMode::Plain);
        assert_eq!(plain, "()\n");
    }

    #[test]
    fn test_format_warning_and_error() {
        let w_term = format_warning("some warning", PresentationMode::Terminal);
        assert_eq!(w_term, "\x1b[33m⚠\x1b[0m \x1b[33msome warning\x1b[0m\n");

        let w_plain = format_warning("some warning", PresentationMode::Plain);
        assert_eq!(w_plain, "warning: some warning\n");

        let e_term = format_error("snap: failed", PresentationMode::Terminal);
        assert_eq!(e_term, "\x1b[31m✗ snap: failed\x1b[0m\n");

        let e_plain = format_error("snap: failed", PresentationMode::Plain);
        assert_eq!(e_plain, "snap: failed\n");
    }
}
