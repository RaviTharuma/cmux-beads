//! Project `bd` issues into cmux `set-status` pills (`bead:<id>`).
//!
//! Custom sidebars cannot spawn `bd`. `cmux-beads sync` / `watch` write the
//! pills; the sidebar only reads `w.statuses`. Writes back to `bd` stay on
//! this CLI (argv), never in the sidebar file.

use std::collections::BTreeMap;

use crate::bd::Bead;

/// Prefix for every projected status key.
pub const STATUS_PREFIX: &str = "bead:";

/// Cap pills so the sidebar stays cheap (cmux custom-sidebar guidance).
pub const MAX_PILLS: usize = 24;

/// Display value budget for `cmux set-status`.
pub const VALUE_MAX: usize = 48;

/// Safe bead id: starts alphanumeric, then `A-Za-z0-9._-`.
const ID_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-";

/// Icon / color / sort weight for one `bd` status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusStyle {
    pub icon: &'static str,
    pub color: &'static str,
    pub priority: u8,
}

/// One planned `cmux set-status` write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusPill {
    pub key: String,
    pub value: String,
    pub icon: String,
    pub color: String,
    pub priority: u8,
}

/// Diff of desired pills versus live `bead:*` keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPlan {
    pub apply: Vec<StatusPill>,
    pub stale: Vec<String>,
    pub counts: BTreeMap<String, u32>,
}

/// Build `bead:<id>` when `id` is a safe argv token.
#[must_use]
pub fn status_key(id: &str) -> Option<String> {
    if !is_safe_id(id) {
        return None;
    }
    Some(format!("{STATUS_PREFIX}{id}"))
}

/// Whether `id` is safe to embed in a `bead:` key.
#[must_use]
pub fn is_safe_id(id: &str) -> bool {
    let first = id.chars().next();
    !id.is_empty()
        && id.len() <= 64
        && first.is_some_and(|ch| ch.is_ascii_alphanumeric())
        && id.chars().all(|ch| ID_CHARS.contains(ch))
}

/// Style for a `bd` status. Unknown names share the open chip.
#[must_use]
pub fn style_for(status: &str) -> StatusStyle {
    match status {
        "in_progress" => StatusStyle {
            icon: "hammer",
            color: "#ff9500",
            priority: 80,
        },
        "blocked" => StatusStyle {
            icon: "exclamationmark.triangle",
            color: "#ff3b30",
            priority: 90,
        },
        "deferred" => StatusStyle {
            icon: "pause.circle",
            color: "#8e8e93",
            priority: 20,
        },
        "pinned" => StatusStyle {
            icon: "pin.fill",
            color: "#ff9500",
            priority: 70,
        },
        "hooked" => StatusStyle {
            icon: "link",
            color: "#bf5af2",
            priority: 50,
        },
        "closed" => StatusStyle {
            icon: "checkmark.circle",
            color: "#8e8e93",
            priority: 10,
        },
        _ => StatusStyle {
            icon: "circle",
            color: "#34c759",
            priority: 40,
        },
    }
}

/// Chip label: `{status} · {title}`. Title only — never assignee, path, or email.
#[must_use]
pub fn pill_value(bead: &Bead) -> String {
    let title = bead.title.trim();
    let raw = if title.is_empty() {
        bead.status.clone()
    } else {
        format!("{} · {title}", bead.status)
    };
    truncate_chars(&raw, VALUE_MAX)
}

/// Project beads into pills. Closed issues are omitted unless requested.
#[must_use]
pub fn pills_from_beads(beads: &[Bead], include_closed: bool) -> Vec<StatusPill> {
    let mut pills = Vec::new();
    for bead in beads {
        if !include_closed && bead.is_closed() {
            continue;
        }
        let Some(key) = status_key(&bead.id) else {
            continue;
        };
        let style = style_for(&bead.status);
        pills.push(StatusPill {
            key,
            value: pill_value(bead),
            icon: style.icon.to_string(),
            color: style.color.to_string(),
            priority: style.priority,
        });
        if pills.len() == MAX_PILLS {
            break;
        }
    }
    pills
}

/// Count pills by the `bd` status prefix of the value.
#[must_use]
pub fn count_statuses(pills: &[StatusPill]) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for pill in pills {
        let status = pill.value.split(" · ").next().unwrap_or("open").to_string();
        *counts.entry(status).or_insert(0) += 1;
    }
    counts
}

/// Desired writes plus stale `bead:*` keys to clear.
#[must_use]
pub fn plan_sync(beads: &[Bead], existing_keys: &[String], include_closed: bool) -> SyncPlan {
    let apply = pills_from_beads(beads, include_closed);
    let desired: Vec<String> = apply.iter().map(|pill| pill.key.clone()).collect();
    SyncPlan {
        counts: count_statuses(&apply),
        stale: stale_keys(existing_keys, &desired),
        apply,
    }
}

/// `cmux set-status` argv for one pill. Workspace is appended by the runner.
#[must_use]
pub fn set_status_argv(pill: &StatusPill) -> Vec<String> {
    vec![
        "set-status".into(),
        pill.key.clone(),
        pill.value.clone(),
        "--icon".into(),
        pill.icon.clone(),
        "--color".into(),
        pill.color.clone(),
        "--priority".into(),
        pill.priority.to_string(),
    ]
}

/// `cmux clear-status` argv for one key.
#[must_use]
pub fn clear_status_argv(key: &str) -> Vec<String> {
    vec!["clear-status".into(), key.into()]
}

/// `cmux list-status` argv.
#[must_use]
pub fn list_status_argv() -> Vec<String> {
    vec!["list-status".into()]
}

/// `cmux set-progress` argv for the in-progress fraction.
#[must_use]
pub fn set_progress_argv(value: f64, label: &str) -> Vec<String> {
    vec![
        "set-progress".into(),
        format!("{value:.3}"),
        "--label".into(),
        label.into(),
    ]
}

/// Progress is in-progress / non-closed pills.
#[must_use]
pub fn progress_from_counts(counts: &BTreeMap<String, u32>) -> Option<(f64, String)> {
    let in_progress = *counts.get("in_progress").unwrap_or(&0);
    let closed = *counts.get("closed").unwrap_or(&0);
    let total: u32 = counts.values().sum();
    let active = total.saturating_sub(closed);
    if active == 0 {
        return None;
    }
    let value = f64::from(in_progress) / f64::from(active);
    Some((value, format!("beads {in_progress}/{active} in progress")))
}

/// Extract `bead:*` keys from `cmux list-status` stdout (`key=value` lines).
#[must_use]
pub fn parse_status_keys(raw: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || !line.contains('=') {
            continue;
        }
        let key = line.split('=').next().unwrap_or("").trim();
        if key.starts_with(STATUS_PREFIX) && is_safe_id(&key[STATUS_PREFIX.len()..]) {
            keys.push(key.to_string());
        }
    }
    keys
}

/// Keys that exist on the workspace but are no longer desired.
#[must_use]
pub fn stale_keys(existing: &[String], desired: &[String]) -> Vec<String> {
    existing
        .iter()
        .filter(|key| key.starts_with(STATUS_PREFIX) && !desired.iter().any(|want| want == *key))
        .cloned()
        .collect()
}

/// Read `workspace_id` / `workspace_ref` from `cmux identify --json`.
#[must_use]
pub fn parse_identify_workspace(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    for section in ["caller", "focused"] {
        if let Some(object) = value.get(section)
            && let Some(id) = workspace_from_object(object)
        {
            return Some(id);
        }
    }
    workspace_from_object(&value)
}

/// `--workspace` wins, then `CMUX_WORKSPACE_ID`. Never invent a host.
#[must_use]
pub fn resolve_workspace(explicit: Option<&str>, env_ws: Option<&str>) -> Option<String> {
    explicit
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            env_ws
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn workspace_from_object(value: &serde_json::Value) -> Option<String> {
    for key in ["workspace_ref", "workspace_id"] {
        if let Some(text) = value.get(key).and_then(|item| item.as_str()) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bd::parse_list;

    fn lab() -> Vec<Bead> {
        parse_list(include_str!("../tests/fixtures/lab.json")).expect("lab.json")
    }

    #[test]
    fn status_key_accepts_safe_ids() {
        assert_eq!(status_key("lab-1").as_deref(), Some("bead:lab-1"));
        assert_eq!(status_key("bd.2_x").as_deref(), Some("bead:bd.2_x"));
        assert!(status_key("").is_none());
        assert!(status_key("-bad").is_none());
        assert!(status_key("has space").is_none());
        assert!(status_key("bead:nested").is_none());
        assert!(status_key("rm;id").is_none());
    }

    #[test]
    fn style_for_known_statuses() {
        assert_eq!(style_for("in_progress").icon, "hammer");
        assert_eq!(style_for("blocked").color, "#ff3b30");
        assert_eq!(style_for("open").icon, "circle");
        assert!(style_for("blocked").priority > style_for("open").priority);
    }

    #[test]
    fn pill_value_is_status_and_title_without_pii() {
        let beads = lab();
        let ship = beads.iter().find(|bead| bead.id == "lab-1").unwrap();
        assert_eq!(pill_value(ship), "open · Ship onboarding");
        let login = beads.iter().find(|bead| bead.id == "lab-2").unwrap();
        assert_eq!(pill_value(login), "in_progress · Fix login timeout");
        for bead in &beads {
            let value = pill_value(bead);
            assert!(!value.contains('@'), "pills must not carry emails");
            assert!(
                !value.contains("/Users/"),
                "pills must not carry home paths"
            );
            assert!(!value.contains("cmux:"), "assignee stays off the pill");
        }
    }

    #[test]
    fn pills_cap_and_skip_closed() {
        let mut beads = lab();
        beads.push(Bead {
            id: "lab-closed".into(),
            title: "Done item".into(),
            status: "closed".into(),
            ..Bead::default()
        });
        let open = pills_from_beads(&beads, false);
        assert!(open.iter().all(|pill| !pill.key.ends_with("lab-closed")));
        let with_closed = pills_from_beads(&beads, true);
        assert!(with_closed.iter().any(|pill| pill.key == "bead:lab-closed"));

        let many: Vec<Bead> = (0..40)
            .map(|i| Bead {
                id: format!("n{i}"),
                title: format!("Item {i}"),
                status: "open".into(),
                ..Bead::default()
            })
            .collect();
        assert_eq!(pills_from_beads(&many, false).len(), MAX_PILLS);
    }

    #[test]
    fn set_status_argv_keeps_value_one_element() {
        let pill = StatusPill {
            key: "bead:lab-2".into(),
            value: "in_progress · Fix login timeout".into(),
            icon: "hammer".into(),
            color: "#ff9500".into(),
            priority: 80,
        };
        let args = set_status_argv(&pill);
        assert_eq!(
            args,
            [
                "set-status",
                "bead:lab-2",
                "in_progress · Fix login timeout",
                "--icon",
                "hammer",
                "--color",
                "#ff9500",
                "--priority",
                "80"
            ]
        );
        assert_eq!(
            clear_status_argv("bead:lab-2"),
            ["clear-status", "bead:lab-2"]
        );
    }

    #[test]
    fn parse_status_keys_keeps_bead_prefix_only() {
        let raw = "\
bead:lab-1=open · Ship onboarding
herdr:p1=working
bead:lab-2=in_progress · Fix login timeout
not-a-key
bead:bad id=no
";
        assert_eq!(parse_status_keys(raw), ["bead:lab-1", "bead:lab-2"]);
    }

    #[test]
    fn stale_keys_drop_missing_beads_only() {
        let stale = stale_keys(
            &["bead:lab-1".into(), "bead:gone".into(), "herdr:x".into()],
            &["bead:lab-1".into()],
        );
        assert_eq!(stale, ["bead:gone"]);
    }

    #[test]
    fn plan_sync_matches_lab_fixture() {
        let beads = lab();
        let plan = plan_sync(&beads, &["bead:old".into()], false);
        assert_eq!(plan.apply.len(), 5);
        assert_eq!(plan.stale, ["bead:old"]);
        assert_eq!(plan.counts.get("open").copied(), Some(2));
        assert_eq!(plan.counts.get("in_progress").copied(), Some(1));
        let (value, label) = progress_from_counts(&plan.counts).unwrap();
        assert!((value - 0.2).abs() < f64::EPSILON);
        assert!(label.contains("1/5"));
    }

    #[test]
    fn resolve_workspace_does_not_guess() {
        assert_eq!(
            resolve_workspace(Some("ws-1"), Some("env")),
            Some("ws-1".into())
        );
        assert_eq!(
            resolve_workspace(None, Some("env-ws")),
            Some("env-ws".into())
        );
        assert_eq!(resolve_workspace(None, None), None);
        assert_eq!(resolve_workspace(Some("  "), Some("")), None);
    }

    #[test]
    fn parse_identify_workspace_reads_caller() {
        let raw = r#"{"caller":{"workspace_id":"ws-a"},"focused":{"workspace_ref":"ws-b"}}"#;
        assert_eq!(parse_identify_workspace(raw).as_deref(), Some("ws-a"));
        assert_eq!(
            parse_identify_workspace(r#"{"workspace_ref":"ws-c"}"#).as_deref(),
            Some("ws-c")
        );
        assert!(parse_identify_workspace("not-json").is_none());
    }
}
