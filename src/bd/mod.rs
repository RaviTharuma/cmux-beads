//! The `bd` bridge: every call is an argv vector, never a shell string.
//!
//! Titles, notes, comments, and close reasons therefore cannot be
//! shell-injected. `bd list --json` (or `bd ready --json`) is the source of
//! truth for the board.

pub mod types;

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

pub use types::Bead;

/// Well-known `bd` statuses, in board order. Custom statuses from `bd list`
/// are appended after these when they appear.
pub const KNOWN_STATUSES: &[&str] = &[
    "open",
    "in_progress",
    "blocked",
    "deferred",
    "pinned",
    "hooked",
    "closed",
];

/// Issue types offered by the create / edit form.
pub const ISSUE_TYPES: &[&str] = &[
    "task", "bug", "feature", "epic", "chore", "spike", "story", "decision",
];

/// How the board should query `bd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMode {
    /// `bd list --json` (optionally plus closed).
    All,
    /// `bd ready --json` — issues with no open blockers.
    Ready,
}

/// Repo `.beads` vs `bd --global` (shared-server database).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Repo,
    Global,
}

impl Scope {
    /// Short label for the header.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Repo => "repo",
            Self::Global => "global",
        }
    }

    /// Flip repo ↔ global.
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Repo => Self::Global,
            Self::Global => Self::Repo,
        }
    }
}

/// Failure from the `bd` subprocess or its JSON parser.
#[derive(Debug)]
pub enum BridgeError {
    /// `bd` is not on PATH and was not found in the usual install locations.
    Missing,
    /// `bd` ran but exited non-zero. `command` is the argv joined for display.
    Failed { command: String, message: String },
    /// stdout was not valid `bd --json` output.
    Parse(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(
                f,
                "bd is not on PATH. Install beads v0.60+ from https://github.com/steveyegge/beads then press r to retry."
            ),
            Self::Failed { command, message } => write!(f, "bd {command}: {message}"),
            Self::Parse(message) => write!(f, "bd json: {message}"),
        }
    }
}

impl std::error::Error for BridgeError {}

/// Resolve the `bd` binary. Prefer PATH, then common Homebrew/usr locations.
#[must_use]
pub fn resolve_bd() -> PathBuf {
    if which_bd().is_some() {
        return PathBuf::from("bd");
    }
    for candidate in [
        "/opt/homebrew/bin/bd",
        "/usr/local/bin/bd",
        "/usr/bin/bd",
        "/home/linuxbrew/.linuxbrew/bin/bd",
    ] {
        if Path::new(candidate).exists() {
            return PathBuf::from(candidate);
        }
    }
    PathBuf::from("bd")
}

fn which_bd() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("bd");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Return `Ok(())` when a `bd` executable can be spawned. Does not require a
/// beads database — that is checked when listing.
pub fn probe_bd() -> Result<(), BridgeError> {
    let bd = resolve_bd();
    match Command::new(&bd).arg("--version").output() {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(BridgeError::Missing),
        Err(err) => Err(BridgeError::Failed {
            command: "--version".to_string(),
            message: err.to_string(),
        }),
    }
}

/// Argv for listing the board. Never includes a shell metacharacter join.
#[must_use]
pub fn argv_list(mode: ListMode) -> Vec<String> {
    match mode {
        ListMode::All => vec!["list".into(), "--json".into()],
        ListMode::Ready => vec!["ready".into(), "--json".into()],
    }
}

/// Argv for listing closed issues (best-effort merge).
#[must_use]
pub fn argv_list_closed() -> Vec<String> {
    vec![
        "list".into(),
        "--status".into(),
        "closed".into(),
        "--json".into(),
    ]
}

/// Argv for `bd show <id> --json`.
#[must_use]
pub fn argv_show(id: &str) -> Vec<String> {
    vec!["show".into(), id.into(), "--json".into()]
}

/// Argv for `bd update <id> --claim`.
#[must_use]
pub fn argv_claim(id: &str) -> Vec<String> {
    vec!["update".into(), id.into(), "--claim".into()]
}

/// Argv for `bd close <id> -r <reason>`. `reason` is one argv element.
#[must_use]
pub fn argv_close(id: &str, reason: &str) -> Vec<String> {
    vec!["close".into(), id.into(), "-r".into(), reason.into()]
}

/// Argv for `bd update <id> -s <status>`.
#[must_use]
pub fn argv_set_status(id: &str, status: &str) -> Vec<String> {
    vec!["update".into(), id.into(), "-s".into(), status.into()]
}

/// Argv for `bd update <id> -p <priority>`.
#[must_use]
pub fn argv_set_priority(id: &str, priority: u8) -> Vec<String> {
    vec![
        "update".into(),
        id.into(),
        "-p".into(),
        priority.to_string(),
    ]
}

/// Argv for `bd update <id> -a <assignee>`.
#[must_use]
pub fn argv_set_assignee(id: &str, assignee: &str) -> Vec<String> {
    vec!["update".into(), id.into(), "-a".into(), assignee.into()]
}

/// Argv for `bd note <id> <text>`.
#[must_use]
pub fn argv_note(id: &str, text: &str) -> Vec<String> {
    vec!["note".into(), id.into(), text.into()]
}

/// Argv for `bd comment <id> <text>`.
#[must_use]
pub fn argv_comment(id: &str, text: &str) -> Vec<String> {
    vec!["comment".into(), id.into(), text.into()]
}

/// Fields for `bd create` / `bd update`. Empty optionals stay out of argv.
#[derive(Debug, Clone)]
pub struct NewBead<'a> {
    pub title: &'a str,
    pub issue_type: &'a str,
    pub priority: u8,
    pub description: &'a str,
    pub assignee: &'a str,
    pub parent: &'a str,
    pub labels: &'a str,
    pub deferred: bool,
}

/// Argv for `bd create`. User strings stay single elements.
#[must_use]
pub fn argv_create(new_bead: &NewBead<'_>) -> Vec<String> {
    let mut args = vec![
        "create".into(),
        new_bead.title.into(),
        "-t".into(),
        new_bead.issue_type.into(),
        "-p".into(),
        new_bead.priority.to_string(),
    ];
    if !new_bead.description.is_empty() {
        args.push("--description".into());
        args.push(new_bead.description.into());
    }
    if !new_bead.assignee.is_empty() {
        args.push("-a".into());
        args.push(new_bead.assignee.into());
    }
    if !new_bead.parent.is_empty() {
        args.push("--parent".into());
        args.push(new_bead.parent.into());
    }
    if !new_bead.labels.is_empty() {
        args.push("-l".into());
        args.push(new_bead.labels.into());
    }
    args
}

/// Argv for `bd update` of core fields. Empty optionals are omitted so an
/// untouched field never wipes existing data.
#[must_use]
pub fn argv_update(id: &str, new_bead: &NewBead<'_>) -> Vec<String> {
    let mut args = vec![
        "update".into(),
        id.into(),
        "--title".into(),
        new_bead.title.into(),
        "-t".into(),
        new_bead.issue_type.into(),
        "-p".into(),
        new_bead.priority.to_string(),
    ];
    if !new_bead.description.is_empty() {
        args.push("--description".into());
        args.push(new_bead.description.into());
    }
    if !new_bead.assignee.is_empty() {
        args.push("-a".into());
        args.push(new_bead.assignee.into());
    }
    if !new_bead.parent.is_empty() {
        args.push("--parent".into());
        args.push(new_bead.parent.into());
    }
    if !new_bead.labels.is_empty() {
        args.push("--set-labels".into());
        args.push(new_bead.labels.into());
    }
    args
}

/// Parse `bd list --json`, `bd ready --json`, or `bd show --json` stdout.
pub fn parse_list(raw: &str) -> Result<Vec<Bead>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        return serde_json::from_str(trimmed).context("parsing bd --json array");
    }
    if trimmed.starts_with('{') {
        let bead: Bead = serde_json::from_str(trimmed).context("parsing bd --json object")?;
        return Ok(vec![bead]);
    }
    bail!("expected a JSON array or object from bd --json")
}

/// Run `bd` with `args` in `cwd`. `args` must already be an argv vector.
pub fn run(cwd: &Path, scope: Scope, args: &[String]) -> Result<String, BridgeError> {
    let bd = resolve_bd();
    let mut cmd = Command::new(&bd);
    if scope == Scope::Global {
        cmd.arg("--global");
        cmd.env("BEADS_DOLT_SHARED_SERVER", "1");
    }
    cmd.args(args);
    cmd.current_dir(cwd);
    if let Ok(path) = std::env::var("PATH") {
        cmd.env(
            "PATH",
            format!("/opt/homebrew/bin:/usr/local/bin:/usr/bin:{path}"),
        );
    }
    let output = cmd.output().map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            BridgeError::Missing
        } else {
            BridgeError::Failed {
                command: args.join(" "),
                message: format!("failed to spawn {}: {err}", bd.display()),
            }
        }
    })?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(BridgeError::Failed {
            command: args.join(" "),
            message: err.trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Load the board from `bd`. Closed issues are merged only when requested.
pub fn load(
    cwd: &Path,
    scope: Scope,
    mode: ListMode,
    include_closed: bool,
) -> Result<Vec<Bead>, BridgeError> {
    let raw = run(cwd, scope, &argv_list(mode))?;
    let mut beads = parse_list(&raw).map_err(|err| BridgeError::Parse(err.to_string()))?;
    if include_closed
        && mode == ListMode::All
        && let Ok(closed_raw) = run(cwd, scope, &argv_list_closed())
        && let Ok(extra) = parse_list(&closed_raw)
    {
        let have: std::collections::HashSet<_> = beads.iter().map(|bead| bead.id.clone()).collect();
        beads.extend(extra.into_iter().filter(|bead| !have.contains(&bead.id)));
    }
    Ok(beads)
}

/// `bd show <id> --json` for the detail pane.
pub fn show(cwd: &Path, scope: Scope, id: &str) -> Result<Option<Bead>, BridgeError> {
    let raw = run(cwd, scope, &argv_show(id))?;
    let beads = parse_list(&raw).map_err(|err| BridgeError::Parse(err.to_string()))?;
    Ok(beads.into_iter().next())
}

pub fn claim(cwd: &Path, scope: Scope, id: &str) -> Result<(), BridgeError> {
    run(cwd, scope, &argv_claim(id)).map(|_| ())
}

pub fn close(cwd: &Path, scope: Scope, id: &str, reason: &str) -> Result<(), BridgeError> {
    run(cwd, scope, &argv_close(id, reason)).map(|_| ())
}

pub fn set_status(cwd: &Path, scope: Scope, id: &str, status: &str) -> Result<(), BridgeError> {
    run(cwd, scope, &argv_set_status(id, status)).map(|_| ())
}

pub fn set_priority(cwd: &Path, scope: Scope, id: &str, priority: u8) -> Result<(), BridgeError> {
    run(cwd, scope, &argv_set_priority(id, priority)).map(|_| ())
}

pub fn set_assignee(cwd: &Path, scope: Scope, id: &str, assignee: &str) -> Result<(), BridgeError> {
    run(cwd, scope, &argv_set_assignee(id, assignee)).map(|_| ())
}

pub fn add_note(cwd: &Path, scope: Scope, id: &str, text: &str) -> Result<(), BridgeError> {
    run(cwd, scope, &argv_note(id, text)).map(|_| ())
}

pub fn add_comment(cwd: &Path, scope: Scope, id: &str, text: &str) -> Result<(), BridgeError> {
    run(cwd, scope, &argv_comment(id, text)).map(|_| ())
}

/// Create a bead. Returns stdout (the new id when `bd` prints it).
pub fn create(cwd: &Path, scope: Scope, new_bead: &NewBead<'_>) -> Result<String, BridgeError> {
    let raw = run(cwd, scope, &argv_create(new_bead))?;
    let id = raw.trim().to_string();
    if new_bead.deferred && !id.is_empty() {
        let _ = set_status(cwd, scope, &id, "deferred");
    }
    Ok(id)
}

/// Update an existing bead's core fields from the form.
pub fn update_bead(
    cwd: &Path,
    scope: Scope,
    id: &str,
    new_bead: &NewBead<'_>,
) -> Result<(), BridgeError> {
    run(cwd, scope, &argv_update(id, new_bead))?;
    if new_bead.deferred {
        let _ = set_status(cwd, scope, id, "deferred");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_new<'a>(title: &'a str) -> NewBead<'a> {
        NewBead {
            title,
            issue_type: "bug",
            priority: 1,
            description: "",
            assignee: "",
            parent: "",
            labels: "",
            deferred: false,
        }
    }

    #[test]
    fn parses_list_fixture() {
        let raw = include_str!("../../tests/fixtures/list.json");
        let beads = parse_list(raw).expect("list.json parses");
        assert_eq!(beads.len(), 3);
        assert_eq!(beads[0].id, "demo-1");
        assert_eq!(beads[0].title, "Add dark-mode toggle");
        assert_eq!(beads[0].status, "open");
        assert_eq!(beads[0].priority, 2);
        assert_eq!(beads[0].issue_type, "feature");
        assert_eq!(beads[0].assignee_display(), "user");
        assert_eq!(beads[1].status, "in_progress");
        assert_eq!(beads[2].priority, 0);
        assert_eq!(beads[0].dependencies[0].other_id(), Some("demo-2"));
    }

    #[test]
    fn parses_ready_fixture() {
        let raw = include_str!("../../tests/fixtures/ready.json");
        let beads = parse_list(raw).expect("ready.json parses");
        assert_eq!(beads.len(), 1);
        assert_eq!(beads[0].id, "demo-2");
    }

    #[test]
    fn parses_show_fixture_with_expanded_dependencies() {
        let raw = include_str!("../../tests/fixtures/show.json");
        let beads = parse_list(raw).expect("show.json parses");
        assert_eq!(beads.len(), 1);
        let dep = &beads[0].dependencies[0];
        assert_eq!(dep.other_id(), Some("demo-2"));
        assert_eq!(dep.title.as_deref(), Some("Design token pipeline"));
        assert!(dep.label().contains("demo-2"));
        assert_eq!(beads[0].note_lines(), ["needs design review"]);
    }

    #[test]
    fn empty_and_whitespace_parse_to_empty() {
        assert!(parse_list("").unwrap().is_empty());
        assert!(parse_list(" \n ").unwrap().is_empty());
    }

    #[test]
    fn parses_single_object() {
        let raw = r#"{"id":"x-1","title":"solo","status":"open","priority":1}"#;
        let beads = parse_list(raw).unwrap();
        assert_eq!(beads[0].id, "x-1");
    }

    #[test]
    fn tolerates_unknown_fields() {
        let raw = r#"[{"id":"x-1","title":"t","status":"open","priority":2,"issue_type":"task","surprise_field":123}]"#;
        let beads = parse_list(raw).unwrap();
        assert_eq!(beads[0].id, "x-1");
        assert_eq!(beads[0].assignee_display(), "-");
    }

    #[test]
    fn rejects_non_json() {
        assert!(parse_list("not json").is_err());
    }

    #[test]
    fn list_argv_is_json_source_of_truth() {
        assert_eq!(argv_list(ListMode::All), ["list", "--json"]);
        assert_eq!(argv_list(ListMode::Ready), ["ready", "--json"]);
    }

    #[test]
    fn write_argv_never_joins_user_text() {
        let reason = r#"done; rm -rf / && echo 'injected'"#;
        let args = argv_close("bd-1", reason);
        assert_eq!(args, ["close", "bd-1", "-r", reason]);
        assert_eq!(args.len(), 4, "reason must stay one argv element");

        let title = r#"fix "quotes" & $HOME"#;
        let create_args = argv_create(&sample_new(title));
        assert_eq!(create_args, ["create", title, "-t", "bug", "-p", "1"]);

        let note = r#"see `rm -rf` && echo"#;
        assert_eq!(argv_note("bd-1", note), ["note", "bd-1", note]);
        assert_eq!(argv_comment("bd-1", note), ["comment", "bd-1", note]);
        assert_eq!(
            argv_set_assignee("bd-1", "cmux:1/12"),
            ["update", "bd-1", "-a", "cmux:1/12"]
        );
    }

    #[test]
    fn create_and_update_include_optional_fields() {
        let new_bead = NewBead {
            title: "t",
            issue_type: "epic",
            priority: 0,
            description: "d",
            assignee: "cmux:1/2",
            parent: "e-1",
            labels: "cmux,ui",
            deferred: true,
        };
        let created = argv_create(&new_bead);
        assert!(created.contains(&"--parent".into()));
        assert!(created.contains(&"-l".into()));
        assert!(created.contains(&"cmux,ui".into()));
        let updated = argv_update("bd-9", &new_bead);
        assert!(updated.contains(&"--set-labels".into()));
        assert!(updated.contains(&"--title".into()));
    }

    #[test]
    fn claim_and_status_argv() {
        assert_eq!(argv_claim("bd-9"), ["update", "bd-9", "--claim"]);
        assert_eq!(
            argv_set_status("bd-9", "in_progress"),
            ["update", "bd-9", "-s", "in_progress"]
        );
        assert_eq!(argv_set_priority("bd-9", 0), ["update", "bd-9", "-p", "0"]);
        assert_eq!(argv_show("bd-9"), ["show", "bd-9", "--json"]);
    }
}
