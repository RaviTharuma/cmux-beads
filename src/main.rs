//! cmux-beads: a keyboard-first Beads (`bd`) board for the cmux right sidebar.
//!
//! Designed to run in the cmux sidebar PTY (`CMUX_SIDEBAR=1`). The board
//! never invents a store; `bd list --json` / `bd ready --json` is the source
//! of truth, and every write is an argv vector.

mod app;
mod bd;
mod board;
mod cwd;
mod form;
mod keys;
mod sessions;
mod ui;

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::Result;
use app::App;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

const POLL_EVERY: Duration = Duration::from_millis(100);

struct Cli {
    cwd: Option<PathBuf>,
    selftest: bool,
}

fn parse_args() -> Result<Cli, i32> {
    let mut cwd = None;
    let mut selftest = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--selftest" => selftest = true,
            "--cwd" => {
                let Some(value) = args.next() else {
                    eprintln!("--cwd needs a path");
                    return Err(2);
                };
                cwd = Some(PathBuf::from(value));
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
    Ok(Cli { cwd, selftest })
}

fn print_help() {
    eprintln!(
        "\
cmux-beads — Beads (bd) board for the cmux right sidebar

Usage:
  cmux-beads [--cwd DIR] [--selftest]

  --cwd DIR     Run bd in DIR instead of the focused pane cwd
  --selftest    Print the board as text and exit (no TUI)

Environment:
  CMUX_TUI_SOCKET / CMUX_MUX_SOCKET   cmux control socket
  CMUX_SIDEBAR=1                      set by cmux when hosted in the sidebar
"
    );
}

fn main() -> ExitCode {
    match parse_args() {
        Ok(cli) => {
            if cli.selftest {
                let app = App::new(cli.cwd);
                print!("{}", app.selftest_text());
                return ExitCode::SUCCESS;
            }
            match run_tui(cli.cwd) {
                Ok(()) => ExitCode::SUCCESS;
                Err(err) => {
                    eprintln!("cmux-beads: {err:#}");
                    ExitCode::from(1)
                }
            }
        }
        Err(0) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code as u8),
    }
}

fn run_tui(cwd: Option<PathBuf>) -> Result<()> {
    // Restore the terminal before the default panic output so a panic never
    // leaves the host terminal (or the cmux sidebar PTY) stuck in raw mode.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_hook(info);
    }));

    let mut terminal = setup_terminal()?;
    let result = event_loop(&mut terminal, cwd);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    cwd: Option<PathBuf>,
) -> Result<()> {
    let mut app = App::new(cwd);
    loop {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        if event::poll(POLL_EVERY)?
            && let Event::Key(key) = event::read()?
            && keys::handle_key(&mut app, key)
        {
            break;
        }

        if app.should_quit {
            break;
        }
        app.tick();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn plugin_manifest_matches_sidebar_contract() {
        let raw = include_str!("../cmux-plugin.toml");
        assert!(raw.contains("kind = \"sidebar\""), "kind must be sidebar");
        assert!(
            raw.contains("name = \"cmux-beads\""),
            "plugin name must be cmux-beads"
        );
        assert!(
            raw.contains("target/release/cmux-beads"),
            "run.command must point at the release binary"
        );
        assert!(
            raw.contains("cargo") && raw.contains("build"),
            "build.command must compile the plugin"
        );
        assert!(
            !raw.contains("cmux-sidebar-beads"),
            "old crate name must be gone"
        );
    }
}
