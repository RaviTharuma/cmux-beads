//! Apply a [`crate::project::SyncPlan`] through the `cmux` CLI.
//!
//! The sidebar never calls this. Status persistence into `bd` stays on argv
//! helpers in [`crate::bd`].
//!
//! `watch` prefers `bd events tail --follow` (Beads ≥1.3.0 + events-journal)
//! and falls back to polling `bd list --json` when the journal is off or `bd`
//! is too old.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::bd::events::{
    self, EventRecord, EventsFollow, EventsUnavailable, event_clears_pill, event_issue,
    parse_event_line, parse_journal_error_line,
};
use crate::bd::{self, ListMode, Scope};
use crate::project::{
    MAX_PILLS, StatusPill, SyncPlan, clear_status_argv, count_focused, list_status_argv,
    parse_identify_workspace, parse_status_keys, pills_from_beads, plan_sync, progress_from_counts,
    resolve_workspace, set_progress_argv, set_status_argv, status_key,
};

/// How the CLI talks to `cmux`. Tests inject a fake.
pub trait CmuxHost {
    /// Run `cmux` with an argv vector. Returns stdout.
    fn run(&self, args: &[String]) -> Result<String>;
}

/// Real `cmux` on PATH.
pub struct ProcessCmux;

impl CmuxHost for ProcessCmux {
    fn run(&self, args: &[String]) -> Result<String> {
        let output = Command::new("cmux")
            .args(args)
            .output()
            .with_context(|| format!("spawn cmux {}", args.join(" ")))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            bail!("cmux {}: {}", args.join(" "), err.trim());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// Options shared by `sync`, `watch`, and `status`.
#[derive(Debug, Clone)]
pub struct SyncOpts {
    pub cwd: Option<std::path::PathBuf>,
    pub workspace: Option<String>,
    pub include_closed: bool,
    pub dry_run: bool,
    pub interval: Duration,
    pub json: bool,
    /// Force the 3s `bd list` poll even when events are available.
    pub force_poll: bool,
}

impl Default for SyncOpts {
    fn default() -> Self {
        Self {
            cwd: None,
            workspace: None,
            include_closed: false,
            dry_run: false,
            interval: Duration::from_secs(3),
            json: false,
            force_poll: false,
        }
    }
}

/// Result of one projection pass.
#[derive(Debug, Clone, Serialize)]
pub struct SyncReport {
    pub workspace: String,
    pub applied: Vec<String>,
    pub stale_cleared: Vec<String>,
    pub counts: std::collections::BTreeMap<String, u32>,
    pub dry_run: bool,
    pub summary: String,
}

/// Resolve the workspace, load `bd`, and project pills.
pub fn sync_once(host: &dyn CmuxHost, opts: &SyncOpts, cwd: &Path) -> Result<SyncReport> {
    let workspace = resolve_or_identify(host, opts.workspace.as_deref())?;
    let beads = bd::load(cwd, Scope::Repo, ListMode::All, opts.include_closed)
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    let existing = match host.run(&with_workspace(&list_status_argv(), &workspace)) {
        Ok(raw) => parse_status_keys(&raw),
        Err(_) if opts.dry_run => Vec::new(),
        Err(err) => return Err(err),
    };
    let plan = plan_sync(&beads, &existing, opts.include_closed);
    apply_plan(host, &workspace, &plan, opts.dry_run)
}

/// Clear every live `bead:*` key on the workspace.
pub fn clear_once(
    host: &dyn CmuxHost,
    workspace: Option<&str>,
    dry_run: bool,
) -> Result<SyncReport> {
    let workspace = resolve_or_identify(host, workspace)?;
    let existing = host
        .run(&with_workspace(&list_status_argv(), &workspace))
        .map(|raw| parse_status_keys(&raw))
        .unwrap_or_default();
    let plan = SyncPlan {
        apply: Vec::new(),
        stale: existing,
        counts: Default::default(),
    };
    apply_plan(host, &workspace, &plan, dry_run)
}

/// Watch loop. Prefers events journal; falls back to polling.
pub fn watch_loop(host: &dyn CmuxHost, opts: &SyncOpts, cwd: &Path) -> Result<()> {
    if !opts.force_poll {
        match watch_events(host, opts, cwd) {
            Ok(()) => return Ok(()),
            Err(WatchFallback::Unavailable(reason)) => {
                eprintln!("cmux-beads watch: {reason}");
            }
            Err(WatchFallback::Fatal(err)) => return Err(err),
        }
    } else {
        eprintln!(
            "cmux-beads watch: --force-poll (bd list every {}s)",
            opts.interval.as_secs()
        );
    }
    watch_poll(host, opts, cwd)
}

enum WatchFallback {
    Unavailable(EventsUnavailable),
    Fatal(anyhow::Error),
}

fn watch_poll(host: &dyn CmuxHost, opts: &SyncOpts, cwd: &Path) -> Result<()> {
    loop {
        let report = sync_once(host, opts, cwd)?;
        if !opts.json {
            eprintln!("{}", report.summary);
        }
        std::thread::sleep(opts.interval);
    }
}

fn watch_events(host: &dyn CmuxHost, opts: &SyncOpts, cwd: &Path) -> Result<(), WatchFallback> {
    let head = events::probe_events(cwd, Scope::Repo).map_err(WatchFallback::Unavailable)?;
    let workspace =
        resolve_or_identify(host, opts.workspace.as_deref()).map_err(WatchFallback::Fatal)?;

    eprintln!("cmux-beads watch: bd events tail --follow (since={head}; Ctrl-C to stop)");

    // Baseline from current bd list, then follow only new mutations.
    let mut pills = baseline_pills(host, opts, cwd, &workspace).map_err(WatchFallback::Fatal)?;
    let mut since = head;
    let mut truncations = 0u32;

    loop {
        let mut follow = EventsFollow::spawn(cwd, Scope::Repo, since)
            .map_err(|err| WatchFallback::Fatal(anyhow::anyhow!("{err}")))?;

        loop {
            let line = match follow.next_line() {
                Ok(Some(line)) => line,
                Ok(None) => {
                    eprintln!("cmux-beads watch: events stream ended; rebuilding");
                    break;
                }
                Err(err) => {
                    return Err(WatchFallback::Fatal(anyhow::anyhow!(
                        "reading bd events: {err}"
                    )));
                }
            };

            if let Some(err) = parse_journal_error_line(&line) {
                if err.is_truncated() {
                    truncations += 1;
                    if truncations > 5 {
                        return Err(WatchFallback::Unavailable(EventsUnavailable::Unsupported(
                            "events journal truncated repeatedly".into(),
                        )));
                    }
                    eprintln!(
                        "cmux-beads watch: journal truncated (floor={:?} head={:?}); rebuilding from bd list",
                        err.floor, err.head
                    );
                    follow.stop();
                    pills = baseline_pills(host, opts, cwd, &workspace)
                        .map_err(WatchFallback::Fatal)?;
                    since = err.head.unwrap_or(since);
                    break;
                }
                eprintln!(
                    "cmux-beads watch: journal error {}; falling back to poll",
                    err.code
                );
                return Err(WatchFallback::Unavailable(EventsUnavailable::Unsupported(
                    err.code,
                )));
            }

            let record = match parse_event_line(&line) {
                Ok(Some(record)) => record,
                Ok(None) => continue,
                Err(err) => {
                    eprintln!("cmux-beads watch: skip line ({err})");
                    continue;
                }
            };
            since = since.max(record.seq);
            match apply_event(host, &workspace, &record, opts, &mut pills) {
                Ok(Some(summary)) if !opts.json => eprintln!("{summary}"),
                Ok(_) => {}
                Err(err) => eprintln!("cmux-beads watch: apply event seq={}: {err}", record.seq),
            }
        }
        // Stream ended or truncate rebuild: re-baseline head if needed and respawn.
        match events::probe_events(cwd, Scope::Repo) {
            Ok(new_head) => since = since.max(new_head),
            Err(reason) => return Err(WatchFallback::Unavailable(reason)),
        }
    }
}

fn baseline_pills(
    host: &dyn CmuxHost,
    opts: &SyncOpts,
    cwd: &Path,
    workspace: &str,
) -> Result<BTreeMap<String, StatusPill>> {
    let report = {
        let beads = bd::load(cwd, Scope::Repo, ListMode::All, opts.include_closed)
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        let existing = match host.run(&with_workspace(&list_status_argv(), workspace)) {
            Ok(raw) => parse_status_keys(&raw),
            Err(_) if opts.dry_run => Vec::new(),
            Err(err) => return Err(err),
        };
        let plan = plan_sync(&beads, &existing, opts.include_closed);
        apply_plan(host, workspace, &plan, opts.dry_run)?
    };
    if !opts.json {
        eprintln!("{} [baseline]", report.summary);
    }
    let beads = bd::load(cwd, Scope::Repo, ListMode::All, opts.include_closed)
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    let mut map = BTreeMap::new();
    for pill in pills_from_beads(&beads, opts.include_closed) {
        let id = pill.key.trim_start_matches("bead:").to_string();
        map.insert(id, pill);
    }
    Ok(map)
}

/// Apply one journal record to a single pill (or clear it).
fn apply_event(
    host: &dyn CmuxHost,
    workspace: &str,
    record: &EventRecord,
    opts: &SyncOpts,
    pills: &mut BTreeMap<String, StatusPill>,
) -> Result<Option<String>> {
    let id = &record.issue_id;
    if event_clears_pill(record, opts.include_closed) {
        let Some(key) = status_key(id) else {
            return Ok(None);
        };
        pills.remove(id);
        if !opts.dry_run {
            let args = with_workspace(&clear_status_argv(&key), workspace);
            let _ = host.run(&args);
        }
        refresh_progress(host, workspace, pills, opts.dry_run)?;
        return Ok(Some(format!(
            "cmux-beads watch: cleared {key} (op={} seq={})",
            record.op, record.seq
        )));
    }

    let Some(issue) = event_issue(record) else {
        // comment / dep without usable snapshot — ignore.
        return Ok(None);
    };

    let Some(pill) = pills_from_beads(std::slice::from_ref(issue), opts.include_closed)
        .into_iter()
        .next()
    else {
        return Ok(None);
    };

    // Respect MAX_PILLS: skip brand-new ids once the board is full.
    if !pills.contains_key(id) && pills.len() >= MAX_PILLS {
        return Ok(Some(format!(
            "cmux-beads watch: skip {} (at MAX_PILLS={MAX_PILLS})",
            pill.key
        )));
    }

    let changed = pills.get(id) != Some(&pill);
    pills.insert(id.clone(), pill.clone());
    if !changed {
        return Ok(None);
    }
    if opts.dry_run {
        return Ok(Some(format!(
            "cmux-beads watch: would set {} (op={} seq={})",
            pill.key, record.op, record.seq
        )));
    }
    let args = with_workspace(&set_status_argv(&pill), workspace);
    host.run(&args)?;
    refresh_progress(host, workspace, pills, false)?;
    Ok(Some(format!(
        "cmux-beads watch: {} → {} (op={} seq={})",
        pill.key, pill.value, record.op, record.seq
    )))
}

fn refresh_progress(
    host: &dyn CmuxHost,
    workspace: &str,
    pills: &BTreeMap<String, StatusPill>,
    dry_run: bool,
) -> Result<()> {
    let counts = crate::project::count_statuses(&pills.values().cloned().collect::<Vec<_>>());
    if let Some((value, label)) = progress_from_counts(&counts) {
        let args = with_workspace(&set_progress_argv(value, &label), workspace);
        if !dry_run {
            let _ = host.run(&args);
        }
    }
    let _ = count_focused(&pills.values().cloned().collect::<Vec<_>>());
    Ok(())
}

fn apply_plan(
    host: &dyn CmuxHost,
    workspace: &str,
    plan: &SyncPlan,
    dry_run: bool,
) -> Result<SyncReport> {
    let mut applied = Vec::new();
    let mut stale_cleared = Vec::new();
    for pill in &plan.apply {
        let args = with_workspace(&set_status_argv(pill), workspace);
        if dry_run {
            println!("cmux {}", args.join(" "));
            applied.push(pill.key.clone());
            continue;
        }
        host.run(&args)?;
        applied.push(pill.key.clone());
    }
    for key in &plan.stale {
        let args = with_workspace(&clear_status_argv(key), workspace);
        if dry_run {
            println!("cmux {}", args.join(" "));
            stale_cleared.push(key.clone());
            continue;
        }
        host.run(&args)?;
        stale_cleared.push(key.clone());
    }
    if let Some((value, label)) = progress_from_counts(&plan.counts) {
        let args = with_workspace(&set_progress_argv(value, &label), workspace);
        if dry_run {
            println!("cmux {}", args.join(" "));
        } else {
            let _ = host.run(&args);
        }
    }
    let summary = format!(
        "cmux-beads sync: {} beads → cmux ws={} ({}{}{})",
        plan.apply.len(),
        workspace,
        format_counts(&plan.counts),
        {
            let focused = crate::project::count_focused(&plan.apply);
            if focused == 0 {
                String::new()
            } else {
                format!(" focus={focused}")
            }
        },
        if stale_cleared.is_empty() {
            String::new()
        } else {
            format!(" stale={}", stale_cleared.len())
        }
    );
    Ok(SyncReport {
        workspace: workspace.to_string(),
        applied,
        stale_cleared,
        counts: plan.counts.clone(),
        dry_run,
        summary,
    })
}

fn resolve_or_identify(host: &dyn CmuxHost, explicit: Option<&str>) -> Result<String> {
    if let Some(ws) =
        resolve_workspace(explicit, std::env::var("CMUX_WORKSPACE_ID").ok().as_deref())
    {
        return Ok(ws);
    }
    if let Ok(raw) = host.run(&["identify".into(), "--json".into()])
        && let Some(ws) = parse_identify_workspace(&raw)
    {
        return Ok(ws);
    }
    bail!(
        "could not resolve cmux workspace (pass --workspace or set CMUX_WORKSPACE_ID); refusing to guess a host"
    )
}

fn with_workspace(args: &[String], workspace: &str) -> Vec<String> {
    let mut out = args.to_vec();
    out.push("--workspace".into());
    out.push(workspace.into());
    out
}

fn format_counts(counts: &std::collections::BTreeMap<String, u32>) -> String {
    if counts.is_empty() {
        return "empty".into();
    }
    counts
        .iter()
        .map(|(status, n)| format!("{status}={n}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeCmux {
        list: String,
        identify: String,
        calls: RefCell<Vec<Vec<String>>>,
        fail_list: bool,
    }

    impl CmuxHost for FakeCmux {
        fn run(&self, args: &[String]) -> Result<String> {
            self.calls.borrow_mut().push(args.to_vec());
            if args.first().map(String::as_str) == Some("identify") {
                return Ok(self.identify.clone());
            }
            if args.first().map(String::as_str) == Some("list-status") {
                if self.fail_list {
                    bail!("list-status failed");
                }
                return Ok(self.list.clone());
            }
            Ok(String::new())
        }
    }

    #[test]
    fn apply_plan_writes_set_status_and_clears_stale() {
        let host = FakeCmux {
            list: String::new(),
            identify: String::new(),
            calls: RefCell::new(Vec::new()),
            fail_list: false,
        };
        let beads = bd::parse_list(include_str!("../tests/fixtures/lab.json")).unwrap();
        let plan = crate::project::plan_sync(&beads, &["bead:old".into()], false);
        let report = apply_plan(&host, "ws-1", &plan, false).unwrap();
        assert_eq!(report.workspace, "ws-1");
        assert!(report.applied.iter().any(|key| key == "bead:lab-1"));
        assert_eq!(report.stale_cleared, ["bead:old"]);
        let calls = host.calls.borrow();
        assert!(
            calls
                .iter()
                .any(|args| args[0] == "set-status" && args.contains(&"--workspace".into()))
        );
        assert!(calls.iter().any(
            |args| args.first().map(String::as_str) == Some("clear-status")
                && args[1] == "bead:old"
        ));
        assert!(
            !calls
                .iter()
                .any(|args| args.iter().any(|part| part.contains("/Users/")))
        );
    }

    #[test]
    fn dry_run_does_not_call_host() {
        let host = FakeCmux {
            list: String::new(),
            identify: String::new(),
            calls: RefCell::new(Vec::new()),
            fail_list: false,
        };
        let beads = bd::parse_list(include_str!("../tests/fixtures/lab.json")).unwrap();
        let plan = crate::project::plan_sync(&beads, &[], false);
        apply_plan(&host, "ws-1", &plan, true).unwrap();
        assert!(host.calls.borrow().is_empty());
    }

    #[test]
    fn resolve_or_identify_uses_identify_when_env_missing() {
        let host = FakeCmux {
            list: String::new(),
            identify: r#"{"caller":{"workspace_id":"from-identify"}}"#.into(),
            calls: RefCell::new(Vec::new()),
            fail_list: false,
        };
        assert_eq!(
            resolve_or_identify(&host, Some("explicit")).unwrap(),
            "explicit"
        );
    }

    #[test]
    fn apply_event_updates_single_pill() {
        let host = FakeCmux {
            list: String::new(),
            identify: String::new(),
            calls: RefCell::new(Vec::new()),
            fail_list: false,
        };
        let mut pills = BTreeMap::new();
        let opts = SyncOpts {
            workspace: Some("ws-1".into()),
            ..SyncOpts::default()
        };
        let record = events::parse_event_line(
            r#"{"seq":4,"op":"update","issue_id":"lab-2","issue":{"id":"lab-2","title":"Fix login","status":"in_progress","priority":1}}"#,
        )
        .unwrap()
        .unwrap();
        let summary = apply_event(&host, "ws-1", &record, &opts, &mut pills)
            .unwrap()
            .unwrap();
        assert!(summary.contains("bead:lab-2"));
        assert!(pills.contains_key("lab-2"));
        let calls = host.calls.borrow();
        assert!(calls.iter().any(
            |args| args.first().map(String::as_str) == Some("set-status")
                && args[1] == "bead:lab-2"
        ));
    }

    #[test]
    fn apply_event_clears_on_delete() {
        let host = FakeCmux {
            list: String::new(),
            identify: String::new(),
            calls: RefCell::new(Vec::new()),
            fail_list: false,
        };
        let mut pills = BTreeMap::new();
        pills.insert(
            "lab-1".into(),
            StatusPill {
                key: "bead:lab-1".into(),
                value: "open · x".into(),
                icon: "circle".into(),
                color: "#34c759".into(),
                priority: 40,
            },
        );
        let opts = SyncOpts::default();
        let record =
            events::parse_event_line(r#"{"seq":11,"op":"delete","issue_id":"lab-1","issue":null}"#)
                .unwrap()
                .unwrap();
        apply_event(&host, "ws-1", &record, &opts, &mut pills).unwrap();
        assert!(!pills.contains_key("lab-1"));
        assert!(
            host.calls
                .borrow()
                .iter()
                .any(|args| args.first().map(String::as_str) == Some("clear-status"))
        );
    }
}
