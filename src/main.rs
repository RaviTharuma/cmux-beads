//! cmux-beads: native Beads sidebar for cmux, plus a keyboard-only PTY fallback.
//!
//! Product path: install `sidebars/beads.js`, `cmux right-sidebar set custom beads`,
//! and `cmux-beads watch` to project `bd` issues into `bead:<id>` status pills.
//! The PTY TUI remains for `cmux sidebar plugin install` (no mouse).

mod app;
mod bd;
mod board;
mod cli;
mod cwd;
mod form;
mod install;
mod keys;
mod project;
mod sessions;
mod sync;
mod ui;

use std::env;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::Result;
use app::App;
use cli::Command;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use sync::{ProcessCmux, SyncOpts};

const POLL_EVERY: Duration = Duration::from_millis(100);

fn main() -> ExitCode {
    // The plugin manager hosts this binary in a PTY. That path is keyboard-only
    // and must not be mistaken for the native custom sidebar.
    if cli::hosted_in_pty_sidebar() {
        return match dispatch(Command::Tui {
            cwd: None,
            selftest: false,
        }) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("cmux-beads: {err:#}");
                ExitCode::from(1)
            }
        };
    }
    match cli::parse_args(env::args().skip(1)) {
        Ok(Command::Help) => {
            cli::print_help();
            ExitCode::SUCCESS
        }
        Ok(command) => match dispatch(command) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("cmux-beads: {err:#}");
                ExitCode::from(1)
            }
        },
        Err(0) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code as u8),
    }
}

fn dispatch(command: Command) -> Result<()> {
    match command {
        Command::Help => {
            cli::print_help();
            Ok(())
        }
        Command::Tui { cwd, selftest } => {
            if selftest {
                let app = App::new(cwd);
                print!("{}", app.selftest_text());
                return Ok(());
            }
            run_tui(cwd)
        }
        Command::Sync(opts) => run_sync(opts),
        Command::Watch(opts) => {
            let cwd = resolve_cwd(opts.cwd.clone());
            eprintln!(
                "cmux-beads watch: interval {}s (Ctrl-C to stop)",
                opts.interval.as_secs()
            );
            sync::watch_loop(&ProcessCmux, &opts, &cwd)
        }
        Command::Status(opts) => run_sync(opts),
        Command::Clear {
            workspace,
            dry_run,
            json,
        } => {
            let report = sync::clear_once(&ProcessCmux, workspace.as_deref(), dry_run)?;
            print_report(&report, json);
            Ok(())
        }
        Command::Install => run_install(),
        Command::Update {
            id,
            status,
            claim,
            cwd,
            workspace,
        } => run_update(id, status, claim, cwd, workspace),
    }
}

fn run_sync(opts: SyncOpts) -> Result<()> {
    let cwd = resolve_cwd(opts.cwd.clone());
    let report = sync::sync_once(&ProcessCmux, &opts, &cwd)?;
    print_report(&report, opts.json);
    Ok(())
}

fn print_report(report: &sync::SyncReport, json: bool) {
    if json {
        match serde_json::to_string(report) {
            Ok(raw) => println!("{raw}"),
            Err(err) => eprintln!("cmux-beads: json encode failed: {err}"),
        }
        return;
    }
    println!("{}", report.summary);
}

fn run_install() -> Result<()> {
    let source = install::source_dir().ok_or_else(|| {
        anyhow::anyhow!(
            "could not find sidebars/beads.js (set CMUX_BEADS_SHARE or run from the repo)"
        )
    })?;
    let dest = install::dest_dir();
    let written = install::install_sidebars(&source, &dest)?;
    println!("cmux-beads install");
    for path in written {
        println!("  sidebar: {}", path.display());
    }
    println!();
    println!("Next steps:");
    println!("  cmux right-sidebar set custom beads");
    println!("  cmux-beads watch");
    println!();
    println!("Keyboard-only fallback: cmux sidebar plugin use cmux-beads");
    Ok(())
}

fn run_update(
    id: String,
    status: Option<String>,
    claim: bool,
    cwd: Option<PathBuf>,
    workspace: Option<String>,
) -> Result<()> {
    let cwd = resolve_cwd(cwd);
    if claim {
        bd::claim(&cwd, bd::Scope::Repo, &id).map_err(|err| anyhow::anyhow!("{err}"))?;
        eprintln!("cmux-beads update: claimed {id}");
    }
    if let Some(status) = status {
        bd::set_status(&cwd, bd::Scope::Repo, &id, &status)
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        eprintln!("cmux-beads update: {id} → {status}");
    }
    let opts = SyncOpts {
        workspace,
        ..SyncOpts::default()
    };
    if let Ok(report) = sync::sync_once(&ProcessCmux, &opts, &cwd) {
        println!("{}", report.summary);
    }
    Ok(())
}

fn resolve_cwd(forced: Option<PathBuf>) -> PathBuf {
    if let Some(path) = forced {
        return path;
    }
    if let Some(socket) = cwd::socket_from_env()
        && let Ok(mut client) = cwd::connect(socket)
        && let Ok(path) = cwd::resolve_focused_cwd(&mut client)
    {
        return path;
    }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
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

        if event::poll(POLL_EVERY)? {
            match event::read()? {
                Event::Key(key) => {
                    if keys::handle_key(&mut app, key) {
                        break;
                    }
                }
                // Sidebar PTYs get SIGWINCH as a normal resize. Mouse is not
                // forwarded to plugins — do not enable capture or invent DnD.
                Event::Resize(_, _) => {}
                Event::Mouse(_) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
            }
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
            raw.contains("version = \"0.2.0\""),
            "plugin version must be 0.2.0"
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

    #[test]
    fn event_loop_does_not_enable_mouse_or_invent_dnd() {
        let code = include_str!("main.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("main.rs has a test module");
        assert!(
            !code.contains("MouseCapture"),
            "PTY sidebar plugins do not receive mouse"
        );
        assert!(
            code.contains("Event::Resize"),
            "sidebar PTYs observe SIGWINCH as a normal resize"
        );
    }

    #[test]
    fn native_js_sidebar_is_the_product() {
        let js = include_str!("../sidebars/beads.js");
        assert!(js.contains("sidebar("));
        assert!(js.contains("Beads"));
        assert!(js.contains("official GUI"));
        assert!(js.contains("not an iframe"));
        assert!(js.contains("Reorderable"));
        assert!(js.contains("workspace.reorder"));
        assert!(js.contains("workspace.select"));
        assert!(js.contains("surface.focus"));
        assert!(js.contains("bead:"));
        assert!(js.contains("\"accent\""));
        assert!(js.contains("\"primary\""));
        assert!(js.contains("\"secondary\""));
        assert!(js.contains("\"tertiary\""));
        assert!(!js.contains("spawn"));
        assert!(!js.contains("require("));
        assert!(!js.contains("fetch("));
        assert!(!js.to_ascii_lowercase().contains("bd list"));
        assert!(!js.contains("Ravi"));
        assert!(!js.contains("/Users/"));
        assert!(!js.contains("@"));
        assert!(!js.contains("Ship onboarding"));
        assert!(!js.contains("Fix login timeout"));
    }

    #[test]
    fn native_swift_sidebar_matches_herdr_shape() {
        let swift = include_str!("../sidebars/beads.swift");
        assert!(swift.contains("Text(\"Beads\")"));
        assert!(swift.contains("Reorderable(workspaces.prefix(40), move: \"workspace.reorder\")"));
        assert!(swift.contains("workspace.select"));
        assert!(swift.contains("surface.focus"));
        assert!(swift.contains("bead:"));
        assert!(swift.contains("\"accent\""));
        assert!(!swift.contains("Ravi"));
        assert!(!swift.contains("/Users/"));
        assert!(!swift.contains("Ship onboarding"));
    }

    #[test]
    fn changelog_and_crate_are_0_2_0() {
        assert!(include_str!("../CHANGELOG.md").contains("## [0.2.0]"));
        assert!(include_str!("../Cargo.toml").contains("version = \"0.2.0\""));
    }

    #[test]
    fn readme_has_no_invented_sidebar_shots() {
        let readme = include_str!("../README.md");
        assert!(!readme.contains(".png"));
        assert!(readme.contains("official"));
        assert!(readme.contains("not an iframe"));
        assert!(readme.contains("cmux right-sidebar set custom beads"));
        assert!(readme.contains("cmux-beads watch"));
    }
}
