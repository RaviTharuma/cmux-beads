//! Subcommand parsing for the product CLI and the keyboard-only TUI fallback.

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use crate::sync::SyncOpts;

/// Parsed invocation.
#[derive(Debug, Clone)]
pub enum Command {
    /// PTY / standalone TUI (keyboard-only fallback).
    Tui {
        cwd: Option<PathBuf>,
        selftest: bool,
    },
    Sync(SyncOpts),
    Watch(SyncOpts),
    Clear {
        workspace: Option<String>,
        dry_run: bool,
        json: bool,
    },
    Install,
    Status(SyncOpts),
    Update {
        id: String,
        status: Option<String>,
        claim: bool,
        cwd: Option<PathBuf>,
        workspace: Option<String>,
    },
    Help,
}

/// Parse `std::env::args` after the binary name.
pub fn parse_args<I>(args: I) -> Result<Command, i32>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter().peekable();
    let Some(first) = args.peek().cloned() else {
        return Ok(Command::Tui {
            cwd: None,
            selftest: false,
        });
    };
    match first.as_str() {
        "-h" | "--help" | "help" => Ok(Command::Help),
        "sync" => {
            args.next();
            Ok(Command::Sync(parse_sync_opts(&mut args)?))
        }
        "watch" => {
            args.next();
            Ok(Command::Watch(parse_sync_opts(&mut args)?))
        }
        "status" => {
            args.next();
            Ok(Command::Status(parse_sync_opts(&mut args)?))
        }
        "clear" => {
            args.next();
            let opts = parse_sync_opts(&mut args)?;
            Ok(Command::Clear {
                workspace: opts.workspace,
                dry_run: opts.dry_run,
                json: opts.json,
            })
        }
        "install" => {
            args.next();
            if args.next().is_some() {
                eprintln!("install takes no arguments");
                return Err(2);
            }
            Ok(Command::Install)
        }
        "update" => {
            args.next();
            parse_update(&mut args)
        }
        _ if first.starts_with('-') => parse_tui(&mut args),
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            Err(2)
        }
    }
}

/// Rendered user-facing help. Right-sidebar tab paths first; TUI is a
/// keyboard-only fallback, and pane / left-list commands are called out as
/// explicitly not the product.
#[must_use]
pub fn help_text() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!(
        "\
cmux-beads {version} — official Beads tab on the cmux right sidebar

Beads is a tab on the existing right sidebar (sibling of Files / Find / Dock),
not a pane and not the left workspace list.

Product (native UI, mouse / click / drag-and-drop):
  cmux right-sidebar set beads                       host tab (built-in)
  cmux sidebar plugin install https://github.com/RaviTharuma/cmux-beads.git
  cmux sidebar plugin use cmux-beads                 plugin package
  cmux-beads watch                                   project bd → bead:<id> pills

Commands:
  sync      one-shot project bd issues → cmux set-status (bead:<id>)
  watch     loop sync (default 3s)
  status    print the current projection plan
  clear     remove bead:* keys only
  update    persist a bd status change via argv, then refresh pills
  install   contrib/legacy: copy sidebars/beads.js (and .swift) to
            ~/.config/cmux/sidebars/ for the generic Custom slot

Not the product:
  cmux sidebar open beads               opens a pane
  cmux sidebar select beads             replaces the left workspace list
  cmux right-sidebar set custom beads   generic Custom slot, not a Beads tab

Keyboard-only fallback (PTY plugin — no mouse):
  cmux-beads [--cwd DIR] [--selftest]

Flags:
  --cwd DIR            bd working directory (default: focused pane cwd)
  --workspace ID       cmux workspace for set-status (or CMUX_WORKSPACE_ID)
  --include-closed     project closed issues too
"
    )
}

/// Print [`help_text`] to stderr.
pub fn print_help() {
    eprintln!("{}", help_text());
}

fn parse_tui<I>(args: &mut std::iter::Peekable<I>) -> Result<Command, i32>
where
    I: Iterator<Item = String>,
{
    let mut cwd = None;
    let mut selftest = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--selftest" => selftest = true,
            "--cwd" => cwd = Some(require_value(args, "--cwd")?),
            "-h" | "--help" => return Ok(Command::Help),
            other => {
                eprintln!("unknown argument: {other}");
                print_help();
                return Err(2);
            }
        }
    }
    Ok(Command::Tui { cwd, selftest })
}

fn parse_sync_opts<I>(args: &mut std::iter::Peekable<I>) -> Result<SyncOpts, i32>
where
    I: Iterator<Item = String>,
{
    let mut opts = SyncOpts::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cwd" => opts.cwd = Some(require_value(args, "--cwd")?),
            "--workspace" => {
                opts.workspace = Some(require_string(args, "--workspace")?);
            }
            "--include-closed" => opts.include_closed = true,
            "--dry-run" => opts.dry_run = true,
            "--json" => opts.json = true,
            "--interval" => {
                let raw = require_string(args, "--interval")?;
                let secs: u64 = raw.parse().map_err(|_| {
                    eprintln!("--interval needs a number of seconds");
                    2
                })?;
                opts.interval = Duration::from_secs(secs.max(1));
            }
            "-h" | "--help" => {
                print_help();
                return Err(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                print_help();
                return Err(2);
            }
        }
    }
    Ok(opts)
}

fn parse_update<I>(args: &mut std::iter::Peekable<I>) -> Result<Command, i32>
where
    I: Iterator<Item = String>,
{
    let mut id = None;
    let mut status = None;
    let mut claim = false;
    let mut cwd = None;
    let mut workspace = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--status" | "-s" => status = Some(require_string(args, "--status")?),
            "--claim" => claim = true,
            "--cwd" => cwd = Some(require_value(args, "--cwd")?),
            "--workspace" => workspace = Some(require_string(args, "--workspace")?),
            other if other.starts_with('-') => {
                eprintln!("unknown argument: {other}");
                return Err(2);
            }
            other if id.is_none() => id = Some(other.to_string()),
            other => {
                eprintln!("unexpected argument: {other}");
                return Err(2);
            }
        }
    }
    let Some(id) = id else {
        eprintln!("update needs a bead id");
        return Err(2);
    };
    if status.is_none() && !claim {
        eprintln!("update needs --status NAME or --claim");
        return Err(2);
    }
    Ok(Command::Update {
        id,
        status,
        claim,
        cwd,
        workspace,
    })
}

fn require_value<I>(args: &mut I, flag: &str) -> Result<PathBuf, i32>
where
    I: Iterator<Item = String>,
{
    Ok(PathBuf::from(require_string(args, flag)?))
}

fn require_string<I>(args: &mut I, flag: &str) -> Result<String, i32>
where
    I: Iterator<Item = String>,
{
    args.next().ok_or_else(|| {
        eprintln!("{flag} needs a value");
        2
    })
}

/// Whether this process is hosted in the PTY sidebar plugin.
#[must_use]
pub fn hosted_in_pty_sidebar() -> bool {
    env::var_os("CMUX_SIDEBAR").is_some_and(|value| value == "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_string()).collect()
    }

    #[test]
    fn empty_args_are_tui() {
        match parse_args(Vec::new()).unwrap() {
            Command::Tui {
                cwd: None,
                selftest: false,
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn sync_flags() {
        let Command::Sync(opts) = parse_args(args(&[
            "sync",
            "--workspace",
            "ws-1",
            "--dry-run",
            "--json",
        ]))
        .unwrap() else {
            panic!("expected sync");
        };
        assert_eq!(opts.workspace.as_deref(), Some("ws-1"));
        assert!(opts.dry_run);
        assert!(opts.json);
    }

    #[test]
    fn update_requires_write() {
        assert!(parse_args(args(&["update", "lab-1"])).is_err());
        let Command::Update {
            id, status, claim, ..
        } = parse_args(args(&["update", "lab-1", "--status", "in_progress"])).unwrap()
        else {
            panic!("expected update");
        };
        assert_eq!(id, "lab-1");
        assert_eq!(status.as_deref(), Some("in_progress"));
        assert!(!claim);
    }

    #[test]
    fn help_leads_with_the_right_sidebar_tab() {
        let help = help_text();
        assert!(help.contains("cmux right-sidebar set beads"));
        assert!(help.contains("cmux sidebar plugin install"));
        assert!(help.contains("cmux sidebar plugin use cmux-beads"));
        assert!(help.contains("cmux-beads watch"));
        assert!(help.contains("Keyboard-only fallback"));
    }

    #[test]
    fn help_marks_pane_and_left_list_commands_as_not_the_product() {
        let help = help_text();
        let (_, disclaimed) = help
            .split_once("Not the product:")
            .expect("help lists non-product commands");
        for command in [
            "cmux sidebar open beads",
            "cmux sidebar select beads",
            "cmux right-sidebar set custom beads",
        ] {
            assert_eq!(
                help.matches(command).count(),
                1,
                "{command} must appear only under 'Not the product:'"
            );
            assert!(
                disclaimed.contains(command),
                "{command} must be listed under 'Not the product:'"
            );
        }
    }
}
