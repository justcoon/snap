pub mod ansi;
pub mod formatters;

pub use formatters::{
    format_action_success, format_diff, format_error, format_log, format_status, format_version,
    format_warning, LogRecord, PresentationMode, StatusRow,
};
use is_terminal::IsTerminal;

/// Environment variable to explicitly control ANSI terminal styling (§7.11).
pub const ENV_SNAP_COLOR: &str = "SNAP_COLOR";
/// Standard environment variable to disable ANSI terminal styling (§7.11).
pub const ENV_NO_COLOR: &str = "NO_COLOR";

/// Modes negotiated for stdout and stderr independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamModes {
    pub stdout: PresentationMode,
    pub stderr: PresentationMode,
}

/// Negotiate presentation mode according to SPEC.md §7.11.
pub fn negotiate_presentation(
    snap_color: Option<&str>,
    no_color_present: bool,
    stdout_is_tty: bool,
    stderr_is_tty: bool,
) -> Result<StreamModes, String> {
    match snap_color {
        None | Some("auto") => {
            if no_color_present {
                Ok(StreamModes {
                    stdout: PresentationMode::Plain,
                    stderr: PresentationMode::Plain,
                })
            } else {
                Ok(StreamModes {
                    stdout: if stdout_is_tty {
                        PresentationMode::Terminal
                    } else {
                        PresentationMode::Plain
                    },
                    stderr: if stderr_is_tty {
                        PresentationMode::Terminal
                    } else {
                        PresentationMode::Plain
                    },
                })
            }
        }
        Some("always") => Ok(StreamModes {
            stdout: PresentationMode::Terminal,
            stderr: PresentationMode::Terminal,
        }),
        Some("never") => Ok(StreamModes {
            stdout: PresentationMode::Plain,
            stderr: PresentationMode::Plain,
        }),
        Some(_) => Err(format!("{ENV_SNAP_COLOR} must be auto, always, or never")),
    }
}

/// Resolve stream modes from the active process environment and terminal state.
pub fn current_stream_modes() -> Result<StreamModes, String> {
    let snap_color = std::env::var(ENV_SNAP_COLOR).ok();
    let no_color = std::env::var_os(ENV_NO_COLOR).is_some();
    let stdout_is_tty = std::io::stdout().is_terminal();
    let stderr_is_tty = std::io::stderr().is_terminal();

    negotiate_presentation(
        snap_color.as_deref(),
        no_color,
        stdout_is_tty,
        stderr_is_tty,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negotiate_presentation() {
        // 1. auto / unset, no NO_COLOR, non-TTY
        let m1 = negotiate_presentation(None, false, false, false).unwrap();
        assert_eq!(m1.stdout, PresentationMode::Plain);
        assert_eq!(m1.stderr, PresentationMode::Plain);

        // 2. auto / unset, no NO_COLOR, TTY stdout
        let m2 = negotiate_presentation(Some("auto"), false, true, false).unwrap();
        assert_eq!(m2.stdout, PresentationMode::Terminal);
        assert_eq!(m2.stderr, PresentationMode::Plain);

        // 3. auto / unset, NO_COLOR present, TTY stdout
        let m3 = negotiate_presentation(None, true, true, true).unwrap();
        assert_eq!(m3.stdout, PresentationMode::Plain);
        assert_eq!(m3.stderr, PresentationMode::Plain);

        // 4. always, even with NO_COLOR and non-TTY
        let m4 = negotiate_presentation(Some("always"), true, false, false).unwrap();
        assert_eq!(m4.stdout, PresentationMode::Terminal);
        assert_eq!(m4.stderr, PresentationMode::Terminal);

        // 5. never, even with TTY
        let m5 = negotiate_presentation(Some("never"), false, true, true).unwrap();
        assert_eq!(m5.stdout, PresentationMode::Plain);
        assert_eq!(m5.stderr, PresentationMode::Plain);

        // 6. invalid value
        let err = negotiate_presentation(Some("sometimes"), false, false, false).unwrap_err();
        assert_eq!(err, "SNAP_COLOR must be auto, always, or never");
    }
}
