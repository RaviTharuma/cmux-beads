//! `bd events` journal bridge (Beads ≥1.3.0).
//!
//! Prefer `bd events tail --follow` for `cmux-beads watch` when the workspace
//! has `events-journal` enabled. The journal is opt-in and per-clone; sync
//! (`bd dolt pull`) is not journaled — rebuild from `bd list` after truncate
//! or when the journal is off / `bd` is too old.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Deserialize;

use super::types::Bead;
use super::{BridgeError, Scope, resolve_bd, run};

/// Minimum `bd` that ships `bd events`.
pub const MIN_EVENTS_VERSION: (u32, u32, u32) = (1, 3, 0);

/// One journal record (JSON Lines from `bd events tail`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct EventRecord {
    pub seq: u64,
    #[serde(default)]
    pub ts: Option<String>,
    pub op: String,
    pub issue_id: String,
    #[serde(default)]
    pub actor: Option<String>,
    /// Full issue after the mutation; `null` on delete.
    #[serde(default)]
    pub issue: Option<Bead>,
}

/// Machine-readable journal failure (truncate mid-stream or `--json` error).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct JournalError {
    pub code: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub since: Option<u64>,
    #[serde(default)]
    pub floor: Option<u64>,
    #[serde(default)]
    pub head: Option<u64>,
}

impl JournalError {
    #[must_use]
    pub fn is_truncated(&self) -> bool {
        self.code == "events_journal_truncated"
    }
}

/// Why events-based watch is unavailable (use poll fallback).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventsUnavailable {
    /// `bd` missing or older than 1.3.0.
    Version { found: String },
    /// `bd events` not recognized.
    NoCommand,
    /// Journal off for this workspace (`bd config set events-journal true`).
    JournalDisabled,
    /// Storage/backend does not support the journal.
    Unsupported(String),
}

impl std::fmt::Display for EventsUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Version { found } => {
                write!(f, "bd {found} < 1.3.0 (no events journal); polling")
            }
            Self::NoCommand => write!(f, "bd has no `events` command; polling"),
            Self::JournalDisabled => write!(
                f,
                "events journal disabled (bd config set events-journal true); polling"
            ),
            Self::Unsupported(msg) => {
                write!(f, "events journal unsupported: {msg}; polling")
            }
        }
    }
}

/// Parsed `bd` semver (ignores git metadata after the numbers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BdVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl BdVersion {
    #[must_use]
    pub fn supports_events(self) -> bool {
        (self.major, self.minor, self.patch) >= MIN_EVENTS_VERSION
    }
}

impl std::fmt::Display for BdVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Parse `bd version 1.3.0 (...)` or `bd version 1.2.2`.
#[must_use]
pub fn parse_bd_version(raw: &str) -> Option<BdVersion> {
    let trimmed = raw.trim();
    let after_version = trimmed
        .split_whitespace()
        .skip_while(|tok| !tok.eq_ignore_ascii_case("version"))
        .nth(1);
    let candidate = after_version.unwrap_or(trimmed);
    let digits: String = candidate
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    let mut parts = digits.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Some(BdVersion {
        major,
        minor,
        patch,
    })
}

/// Argv for `bd events tail --since N` (optional `--follow` / `--limit`).
#[must_use]
pub fn argv_events_tail(since: u64, follow: bool, limit: Option<u32>) -> Vec<String> {
    let mut args = vec![
        "events".into(),
        "tail".into(),
        "--since".into(),
        since.to_string(),
    ];
    if let Some(limit) = limit {
        args.push("--limit".into());
        args.push(limit.to_string());
    }
    if follow {
        args.push("--follow".into());
    }
    args
}

/// Classify a stdout/stderr line that is not a normal event record.
#[must_use]
pub fn parse_journal_error_line(raw: &str) -> Option<JournalError> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let err: JournalError = serde_json::from_str(trimmed).ok()?;
    if err.code.is_empty() {
        return None;
    }
    Some(err)
}

/// Parse one JSONL event line. Returns `None` for empty lines.
pub fn parse_event_line(raw: &str) -> Result<Option<EventRecord>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if let Some(err) = parse_journal_error_line(trimmed) {
        return Err(format!(
            "journal error {}: {}",
            err.code,
            err.error.unwrap_or_default()
        ));
    }
    let record: EventRecord =
        serde_json::from_str(trimmed).map_err(|err| format!("event json: {err}"))?;
    Ok(Some(record))
}

/// Highest `seq` in a batch of JSONL (ignores truncate / non-event lines).
#[must_use]
pub fn max_seq_from_jsonl(raw: &str) -> Option<u64> {
    let mut max = None;
    for line in raw.lines() {
        if let Ok(Some(record)) = parse_event_line(line) {
            max = Some(max.map_or(record.seq, |m: u64| m.max(record.seq)));
        }
    }
    max
}

/// Read `bd` version string via `--version` / `version`.
pub fn read_bd_version(cwd: &Path) -> Result<BdVersion, BridgeError> {
    let bd = resolve_bd();
    for args in [["--version"], ["version"]] {
        let output = Command::new(&bd)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|err| {
                if err.kind() == std::io::ErrorKind::NotFound {
                    BridgeError::Missing
                } else {
                    BridgeError::Failed {
                        command: args.join(" "),
                        message: err.to_string(),
                    }
                }
            })?;
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if let Some(ver) = parse_bd_version(&combined) {
            return Ok(ver);
        }
    }
    Err(BridgeError::Parse(
        "could not parse bd version (need ≥1.3.0 for events)".into(),
    ))
}

fn stderr_journal_disabled(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("events journal is disabled")
        || (lower.contains("events-journal") && lower.contains("disabled"))
}

fn nonempty_or(primary: &str, fallback: &str) -> String {
    let t = primary.trim();
    if t.is_empty() {
        fallback.trim().to_string()
    } else {
        t.to_string()
    }
}

/// Probe whether events-based watch can run in `cwd`.
///
/// On success returns the current journal head (highest retained `seq`), used
/// as the `--since` checkpoint after a full `bd list` baseline.
pub fn probe_events(cwd: &Path, scope: Scope) -> Result<u64, EventsUnavailable> {
    let version = read_bd_version(cwd).map_err(|err| match err {
        BridgeError::Missing => EventsUnavailable::Version {
            found: "missing".into(),
        },
        other => EventsUnavailable::Version {
            found: other.to_string(),
        },
    })?;
    if !version.supports_events() {
        return Err(EventsUnavailable::Version {
            found: version.to_string(),
        });
    }

    let args = argv_events_tail(0, false, Some(1));
    let bd = resolve_bd();
    let mut cmd = Command::new(&bd);
    if scope == Scope::Global {
        cmd.arg("--global");
        cmd.env("BEADS_DOLT_SHARED_SERVER", "1");
    }
    cmd.args(&args).current_dir(cwd);
    let output = cmd.output().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            EventsUnavailable::Version {
                found: "missing".into(),
            }
        } else {
            EventsUnavailable::Unsupported(err.to_string())
        }
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        let combined = format!("{stdout}{stderr}");
        if combined.contains("unknown command") {
            return Err(EventsUnavailable::NoCommand);
        }
        if let Some(err) = parse_journal_error_line(stdout.lines().last().unwrap_or(""))
            && err.is_truncated()
        {
            return Ok(err.head.unwrap_or(0));
        }
        if stderr_journal_disabled(&stderr) {
            return Err(EventsUnavailable::JournalDisabled);
        }
        if combined
            .to_ascii_lowercase()
            .contains("does not support the events journal")
        {
            return Err(EventsUnavailable::Unsupported(stderr.trim().into()));
        }
        return Err(EventsUnavailable::Unsupported(nonempty_or(
            &stderr, &stdout,
        )));
    }

    if stderr_journal_disabled(&stderr) {
        return Err(EventsUnavailable::JournalDisabled);
    }

    let head = match run(cwd, scope, &argv_events_tail(0, false, None)) {
        Ok(raw) => max_seq_from_jsonl(&raw).unwrap_or(0),
        Err(_) => max_seq_from_jsonl(&stdout).unwrap_or(0),
    };
    Ok(head)
}

/// Spawn `bd events tail --since N --follow` and yield JSONL lines.
pub struct EventsFollow {
    child: std::process::Child,
    reader: BufReader<std::process::ChildStdout>,
}

impl EventsFollow {
    /// Start following after `since` (records with seq greater than this).
    pub fn spawn(cwd: &Path, scope: Scope, since: u64) -> Result<Self, BridgeError> {
        let bd = resolve_bd();
        let args = argv_events_tail(since, true, None);
        let mut cmd = Command::new(&bd);
        if scope == Scope::Global {
            cmd.arg("--global");
            cmd.env("BEADS_DOLT_SHARED_SERVER", "1");
        }
        cmd.args(&args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                BridgeError::Missing
            } else {
                BridgeError::Failed {
                    command: args.join(" "),
                    message: err.to_string(),
                }
            }
        })?;
        let stdout = child.stdout.take().ok_or_else(|| BridgeError::Failed {
            command: args.join(" "),
            message: "missing stdout pipe".into(),
        })?;
        Ok(Self {
            child,
            reader: BufReader::new(stdout),
        })
    }

    /// Read the next stdout line (blocks). `None` when the child closes stdout.
    pub fn next_line(&mut self) -> std::io::Result<Option<String>> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        Ok(Some(line))
    }

    /// Best-effort kill on drop / rebuild.
    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for EventsFollow {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Whether this event should clear the pill (delete, or close when hidden).
#[must_use]
pub fn event_clears_pill(record: &EventRecord, include_closed: bool) -> bool {
    if record.op == "delete" {
        return true;
    }
    if let Some(issue) = &record.issue {
        if !include_closed && issue.is_closed() {
            return true;
        }
    } else if record.op == "close" && !include_closed {
        return true;
    }
    false
}

/// Issue snapshot to project, if this event carries one worth applying.
#[must_use]
pub fn event_issue(record: &EventRecord) -> Option<&Bead> {
    record.issue.as_ref().filter(|bead| !bead.id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_semver_from_bd_version_output() {
        assert_eq!(
            parse_bd_version("bd version 1.3.0 (f45b249ce: HEAD@f45b)"),
            Some(BdVersion {
                major: 1,
                minor: 3,
                patch: 0
            })
        );
        assert_eq!(
            parse_bd_version("bd version 1.2.2 (6c124203e: HEAD@6c124)"),
            Some(BdVersion {
                major: 1,
                minor: 2,
                patch: 2
            })
        );
        assert!(
            !parse_bd_version("bd version 1.2.2")
                .unwrap()
                .supports_events()
        );
        assert!(
            parse_bd_version("bd version 1.3.0")
                .unwrap()
                .supports_events()
        );
        assert!(
            parse_bd_version("bd version 2.0.0")
                .unwrap()
                .supports_events()
        );
    }

    #[test]
    fn parses_event_and_truncate_lines() {
        let line = r#"{"seq":3,"ts":"2026-01-02T03:04:05Z","op":"close","issue_id":"demo-399","actor":"worker","issue":{"id":"demo-399","title":"done","status":"closed","priority":1}}"#;
        let record = parse_event_line(line).unwrap().unwrap();
        assert_eq!(record.seq, 3);
        assert_eq!(record.op, "close");
        assert_eq!(record.issue_id, "demo-399");
        assert_eq!(record.issue.as_ref().unwrap().status, "closed");
        assert!(event_clears_pill(&record, false));
        assert!(!event_clears_pill(&record, true));

        let trunc = r#"{"code":"events_journal_truncated","error":"truncated","floor":41,"head":980,"since":12}"#;
        let err = parse_journal_error_line(trunc).unwrap();
        assert!(err.is_truncated());
        assert_eq!(err.head, Some(980));
        assert!(parse_event_line(trunc).is_err());
    }

    #[test]
    fn delete_clears_and_update_keeps() {
        let delete = parse_event_line(
            r#"{"seq":11,"ts":"t","op":"delete","issue_id":"bd-100","issue":null}"#,
        )
        .unwrap()
        .unwrap();
        assert!(event_clears_pill(&delete, false));
        assert!(event_issue(&delete).is_none());

        let update = parse_event_line(
            r#"{"seq":4,"op":"update","issue_id":"bd-100","issue":{"id":"bd-100","title":"wire","status":"open","priority":1,"issue_type":"task","is_blocked":true}}"#,
        )
        .unwrap()
        .unwrap();
        assert!(!event_clears_pill(&update, false));
        assert_eq!(event_issue(&update).unwrap().id, "bd-100");
    }

    #[test]
    fn argv_events_tail_shape() {
        assert_eq!(
            argv_events_tail(4211, true, None),
            ["events", "tail", "--since", "4211", "--follow"]
        );
        assert_eq!(
            argv_events_tail(0, false, Some(1)),
            ["events", "tail", "--since", "0", "--limit", "1"]
        );
    }

    #[test]
    fn max_seq_from_jsonl_skips_junk() {
        let raw = r#"
{"seq":1,"op":"create","issue_id":"a","issue":{"id":"a","title":"t","status":"open","priority":1}}
not json
{"seq":5,"op":"update","issue_id":"a","issue":{"id":"a","title":"t","status":"open","priority":1}}
"#;
        assert_eq!(max_seq_from_jsonl(raw), Some(5));
        assert_eq!(max_seq_from_jsonl(""), None);
    }
}
