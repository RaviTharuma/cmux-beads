//! Apply a [`crate::project::SyncPlan`] through the `cmux` CLI.
//!
//! The sidebar never calls this. Status persistence into `bd` stays on argv
//! helpers in [`crate::bd`].

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::bd::{self, ListMode, Scope};
use crate::project::{
    SyncPlan, clear_status_argv, list_status_argv, parse_identify_workspace, parse_status_keys,
    plan_sync, progress_from_counts, resolve_workspace, set_progress_argv, set_status_argv,
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

/// Watch loop. Returns on the first hard `bd` / workspace failure.
pub fn watch_loop(host: &dyn CmuxHost, opts: &SyncOpts, cwd: &Path) -> Result<()> {
    loop {
        let report = sync_once(host, opts, cwd)?;
        if !opts.json {
            eprintln!("{}", report.summary);
        }
        std::thread::sleep(opts.interval);
    }
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
        "cmux-beads sync: {} beads → cmux ws={} ({}{})",
        plan.apply.len(),
        workspace,
        format_counts(&plan.counts),
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
        // Explicit still wins.
        assert_eq!(
            resolve_or_identify(&host, Some("explicit")).unwrap(),
            "explicit"
        );
    }
}
