//! Application state. All writes go through the `bd` argv bridge; the board
//! never invents its own store.

use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use cmux_client::CmuxClient;

use crate::bd::{self, Bead, BridgeError, ISSUE_TYPES, ListMode, NewBead, Scope};
use crate::board::{
    BoardView, SortKey, adjacent_status, index_of, kanban_columns, status_choices, visible_beads,
};
use crate::cwd;
use crate::form::BeadForm;
use crate::sessions::{self, LivePane, LiveWorkspace};

const REFRESH_EVERY: Duration = Duration::from_secs(2);
const INITIAL_RECONNECT_DELAY: Duration = Duration::from_millis(500);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(8);

/// Modal / prompt layer. `Esc` backs out one layer and never quits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    None,
    Help,
    Detail,
    Filter,
    CloseReason,
    Note,
    Comment,
    StatusPick,
    PriorityPick,
    Create,
    Assign,
}

/// Connection / data health shown in the header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    Ready,
    Standalone,
    Reconnecting { message: String },
    BdMissing,
}

/// Sidebar application. `bd` is the store; this struct is only UI state.
pub struct App {
    pub client: Option<CmuxClient>,
    pub cwd: PathBuf,
    pub cwd_source: String,
    pub beads: Vec<Bead>,
    pub selected: Option<String>,
    pub detail: Option<Bead>,
    pub overlay: Overlay,
    pub prompt: String,
    pub filter: String,
    pub form: BeadForm,
    pub view: BoardView,
    pub sort: SortKey,
    pub scope: Scope,
    pub ready_only: bool,
    pub include_closed: bool,
    pub show_detail_pane: bool,
    pub move_mode: bool,
    pub live_workspaces: Vec<LiveWorkspace>,
    pub live_panes: Vec<LivePane>,
    pub assign_index: usize,
    pub status_msg: String,
    pub health: Health,
    pub should_quit: bool,
    pub g_pending: bool,
    last_refresh: Instant,
    next_reconnect: Instant,
    reconnect_delay: Duration,
    forced_cwd: Option<PathBuf>,
}

impl App {
    /// Build an app. `forced_cwd` overrides pane resolution (CLI `--cwd`).
    pub fn new(forced_cwd: Option<PathBuf>) -> Self {
        let cwd = forced_cwd
            .clone()
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let mut app = Self {
            client: None,
            cwd,
            cwd_source: "process".into(),
            beads: Vec::new(),
            selected: None,
            detail: None,
            overlay: Overlay::None,
            prompt: String::new(),
            filter: String::new(),
            form: BeadForm::new(Vec::new()),
            view: BoardView::List,
            sort: SortKey::Status,
            scope: Scope::Repo,
            ready_only: false,
            include_closed: false,
            show_detail_pane: false,
            move_mode: false,
            live_workspaces: Vec::new(),
            live_panes: Vec::new(),
            assign_index: 0,
            status_msg: String::new(),
            health: Health::Standalone,
            should_quit: false,
            g_pending: false,
            last_refresh: Instant::now() - REFRESH_EVERY,
            next_reconnect: Instant::now(),
            reconnect_delay: INITIAL_RECONNECT_DELAY,
            forced_cwd,
        };
        app.probe_bd_or_mark();
        app.connect_or_schedule();
        app.reload();
        app
    }

    /// Visible rows for the current filter and sort.
    #[must_use]
    pub fn visible(&self) -> Vec<&Bead> {
        visible_beads(&self.beads, &self.active_filter(), self.sort)
            .into_iter()
            .filter(|bead| self.include_closed || !bead.is_closed())
            .collect()
    }

    /// Kanban columns from the current board.
    #[must_use]
    pub fn kanban(&self) -> Vec<(String, Vec<&Bead>)> {
        kanban_columns(&self.beads, &self.active_filter(), self.include_closed)
    }

    fn active_filter(&self) -> String {
        if self.overlay == Overlay::Filter {
            self.prompt.clone()
        } else {
            self.filter.clone()
        }
    }

    /// Selected bead, if it is still on the board.
    #[must_use]
    pub fn selected_bead(&self) -> Option<&Bead> {
        let id = self.selected.as_deref()?;
        self.beads.iter().find(|bead| bead.id == id)
    }

    /// Compact live-tree summary from `CMUX_TUI_SOCKET` (`list-workspaces`).
    /// Empty when the plugin is standalone or the socket has no tree yet.
    #[must_use]
    pub fn live_summary(&self) -> String {
        if self.live_workspaces.is_empty() && self.live_panes.is_empty() {
            return String::new();
        }
        format!(
            "{}ws {}pn",
            self.live_workspaces.len(),
            self.live_panes.len()
        )
    }

    /// Status choices for the `s` picker.
    #[must_use]
    pub fn statuses(&self) -> Vec<String> {
        status_choices(&self.beads)
    }

    fn epics(&self) -> Vec<(String, String)> {
        self.beads
            .iter()
            .filter(|bead| bead.is_epic())
            .map(|bead| (bead.id.clone(), bead.title.clone()))
            .collect()
    }

    /// Periodic reconnect + refresh. Skips reload while a prompt is open so
    /// a mid-type refresh cannot steal the overlay.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if self.client.is_none() && cwd::socket_from_env().is_some() && now >= self.next_reconnect {
            self.connect_or_schedule();
        }
        if self.overlay_blocks_refresh() {
            return;
        }
        if now.duration_since(self.last_refresh) >= REFRESH_EVERY {
            self.sync_cwd();
            self.refresh_panes();
            self.reload();
        }
    }

    fn overlay_blocks_refresh(&self) -> bool {
        matches!(
            self.overlay,
            Overlay::Filter
                | Overlay::CloseReason
                | Overlay::Note
                | Overlay::Comment
                | Overlay::Create
                | Overlay::Assign
                | Overlay::StatusPick
                | Overlay::PriorityPick
        )
    }

    fn probe_bd_or_mark(&mut self) {
        if let Err(BridgeError::Missing) = bd::probe_bd() {
            self.health = Health::BdMissing;
            self.status_msg =
                "bd is not on PATH. Install beads v0.60+ and press r to retry.".into();
        }
    }

    fn connect_or_schedule(&mut self) {
        if self.forced_cwd.is_some() {
            self.cwd_source = "flag".into();
            if !matches!(self.health, Health::BdMissing) {
                self.health = Health::Standalone;
            }
            return;
        }
        let Some(socket) = cwd::socket_from_env() else {
            self.cwd_source = "process".into();
            if !matches!(self.health, Health::BdMissing) {
                self.health = Health::Standalone;
            }
            return;
        };
        match cwd::connect(socket) {
            Ok(mut client) => {
                self.refresh_panes_from(&mut client);
                match cwd::resolve_focused_cwd(&mut client) {
                    Ok(path) => {
                        self.cwd = path;
                        self.cwd_source = "pane".into();
                        self.client = Some(client);
                        self.reconnect_delay = INITIAL_RECONNECT_DELAY;
                        if !matches!(self.health, Health::BdMissing) {
                            self.health = Health::Ready;
                        }
                    }
                    Err(err) => {
                        self.client = Some(client);
                        self.cwd_source = "process".into();
                        if !matches!(self.health, Health::BdMissing) {
                            self.health = Health::Ready;
                        }
                        self.status_msg = format!("cwd unresolved ({err}); using process cwd");
                    }
                }
            }
            Err(err) => self.disconnect_with_backoff(format!("cannot connect to cmux: {err}")),
        }
    }

    fn disconnect_with_backoff(&mut self, message: String) {
        self.client = None;
        if !matches!(self.health, Health::BdMissing) {
            self.health = Health::Reconnecting {
                message: message.clone(),
            };
        }
        self.status_msg = message;
        self.next_reconnect = Instant::now() + self.reconnect_delay;
        self.reconnect_delay = (self.reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
    }

    fn sync_cwd(&mut self) {
        if self.forced_cwd.is_some() {
            return;
        }
        let Some(client) = self.client.as_mut() else {
            return;
        };
        match cwd::resolve_focused_cwd(client) {
            Ok(path) if path != self.cwd => {
                self.cwd = path;
                self.cwd_source = "pane".into();
                self.status_msg = format!("cwd {}", self.cwd.display());
            }
            Ok(_) => {}
            Err(err) => {
                self.disconnect_with_backoff(format!("cmux socket dropped: {err}"));
            }
        }
    }

    fn apply_live_tree(&mut self, tree: &cmux_client::Tree) {
        self.live_workspaces = sessions::flatten_live_workspaces(tree);
        self.live_panes = sessions::flatten_live_panes(tree);
    }

    fn refresh_panes(&mut self) {
        let Some(client) = self.client.as_mut() else {
            return;
        };
        match client.list_workspaces() {
            Ok(tree) => self.apply_live_tree(&tree),
            Err(err) => {
                self.disconnect_with_backoff(format!("cmux socket dropped: {err}"));
            }
        }
    }

    fn refresh_panes_from(&mut self, client: &mut CmuxClient) {
        if let Ok(tree) = client.list_workspaces() {
            self.apply_live_tree(&tree);
        }
    }

    /// Reload the board from `bd`. Selection is kept by id.
    pub fn reload(&mut self) {
        if matches!(self.health, Health::BdMissing) {
            match bd::probe_bd() {
                Ok(()) => {
                    self.health = if self.client.is_some() {
                        Health::Ready
                    } else {
                        Health::Standalone
                    };
                    self.status_msg = "bd found".into();
                }
                Err(BridgeError::Missing) => {
                    self.beads.clear();
                    return;
                }
                Err(err) => {
                    self.status_msg = err.to_string();
                    return;
                }
            }
        }

        let mode = if self.ready_only {
            ListMode::Ready
        } else {
            ListMode::All
        };
        match bd::load(&self.cwd, self.scope, mode, self.include_closed) {
            Ok(beads) => {
                let previous = self.selected.clone();
                self.beads = beads;
                self.last_refresh = Instant::now();
                if let Some(id) = previous {
                    if self.beads.iter().any(|bead| bead.id == id) {
                        self.selected = Some(id);
                    } else {
                        self.selected = self.visible().first().map(|bead| bead.id.clone());
                    }
                } else {
                    self.selected = self.visible().first().map(|bead| bead.id.clone());
                }
                if self.status_msg.is_empty() {
                    self.status_msg = format!(
                        "{} issues from bd {} --json",
                        self.beads.len(),
                        if self.ready_only { "ready" } else { "list" }
                    );
                }
            }
            Err(BridgeError::Missing) => {
                self.health = Health::BdMissing;
                self.beads.clear();
                self.status_msg = BridgeError::Missing.to_string();
            }
            Err(err) => {
                self.status_msg = format!("{} · cwd {}", err, self.cwd.display());
            }
        }
    }

    /// Cycle List / Table / Kanban in-process.
    pub fn cycle_view(&mut self, reverse: bool) {
        self.view = if reverse {
            self.view.prev()
        } else {
            self.view.next()
        };
        self.status_msg = format!("view: {}", self.view.title());
    }

    /// Move the selection by `delta` rows.
    pub fn nav_vert(&mut self, delta: i32) {
        if self.overlay == Overlay::Assign {
            self.nav_assign(delta);
            return;
        }
        if self.view == BoardView::Kanban {
            self.nav_vert_kanban(delta);
            return;
        }
        let rows = self.visible();
        if rows.is_empty() {
            self.selected = None;
            return;
        }
        let current = self
            .selected
            .as_deref()
            .and_then(|id| index_of(&rows, id))
            .unwrap_or(0);
        let next = jump_index(current, rows.len(), delta);
        self.selected = Some(rows[next].id.clone());
    }

    fn nav_vert_kanban(&mut self, delta: i32) {
        let columns = self.kanban();
        let Some(bead) = self.selected_bead() else {
            if let Some((_, cards)) = columns.iter().find(|(_, cards)| !cards.is_empty()) {
                self.selected = cards.first().map(|card| card.id.clone());
            }
            return;
        };
        let Some((_, cards)) = columns.iter().find(|(status, _)| status == &bead.status) else {
            return;
        };
        if cards.is_empty() {
            return;
        }
        let current = cards
            .iter()
            .position(|card| card.id == bead.id)
            .unwrap_or(0);
        let next = jump_index(current, cards.len(), delta);
        self.selected = Some(cards[next].id.clone());
    }

    /// Kanban: change column, or retag when move-mode is on.
    pub fn nav_horiz(&mut self, delta: i32) {
        if self.view != BoardView::Kanban {
            return;
        }
        if self.move_mode {
            self.retag(delta);
            return;
        }
        let columns = self.kanban();
        let Some(bead) = self.selected_bead() else {
            return;
        };
        let Some(next_status) = adjacent_status(&columns, &bead.status, delta) else {
            return;
        };
        let current_idx = columns
            .iter()
            .find(|(status, _)| status == &bead.status)
            .and_then(|(_, cards)| cards.iter().position(|card| card.id == bead.id))
            .unwrap_or(0);
        if let Some((_, cards)) = columns.iter().find(|(status, _)| status == &next_status)
            && let Some(next) = cards.get(current_idx).or(cards.last())
        {
            self.selected = Some(next.id.clone());
        }
    }

    /// `bd update -s` to the adjacent kanban column.
    pub fn retag(&mut self, delta: i32) {
        let columns = self.kanban();
        let Some(bead) = self.selected_bead() else {
            return;
        };
        let Some(next) = adjacent_status(&columns, &bead.status, delta) else {
            return;
        };
        let id = bead.id.clone();
        self.write(format!("status {id} -> {next}"), |cwd, scope| {
            bd::set_status(cwd, scope, &id, &next)
        });
    }

    fn nav_assign(&mut self, delta: i32) {
        if self.live_panes.is_empty() {
            self.assign_index = 0;
            return;
        }
        self.assign_index = jump_index(self.assign_index, self.live_panes.len(), delta);
    }

    pub fn open_help(&mut self) {
        self.overlay = Overlay::Help;
    }

    pub fn open_filter(&mut self) {
        self.prompt = self.filter.clone();
        self.overlay = Overlay::Filter;
    }

    pub fn open_detail(&mut self) {
        let Some(id) = self.selected.clone() else {
            self.status_msg = "nothing selected".into();
            return;
        };
        match bd::show(&self.cwd, self.scope, &id) {
            Ok(Some(bead)) => {
                self.detail = Some(bead);
                self.overlay = Overlay::Detail;
            }
            Ok(None) => self.status_msg = format!("{id}: bd show returned empty"),
            Err(err) => self.status_msg = format!("show {id}: {err}"),
        }
    }

    pub fn toggle_detail_pane(&mut self) {
        self.show_detail_pane = !self.show_detail_pane;
        if self.show_detail_pane {
            self.open_detail_into_pane();
        }
        self.status_msg = if self.show_detail_pane {
            "detail pane on".into()
        } else {
            "detail pane off".into()
        };
    }

    fn open_detail_into_pane(&mut self) {
        if let Some(id) = self.selected.clone()
            && let Ok(Some(bead)) = bd::show(&self.cwd, self.scope, &id)
        {
            self.detail = Some(bead);
        }
    }

    pub fn open_close_reason(&mut self) {
        if self.selected.is_none() {
            self.status_msg = "nothing selected".into();
            return;
        }
        self.prompt.clear();
        self.overlay = Overlay::CloseReason;
    }

    pub fn open_note(&mut self) {
        if self.selected.is_none() {
            self.status_msg = "nothing selected".into();
            return;
        }
        self.prompt.clear();
        self.overlay = Overlay::Note;
    }

    pub fn open_comment(&mut self) {
        if self.selected.is_none() {
            self.status_msg = "nothing selected".into();
            return;
        }
        self.prompt.clear();
        self.overlay = Overlay::Comment;
    }

    pub fn open_status_pick(&mut self) {
        if self.selected.is_none() {
            self.status_msg = "nothing selected".into();
            return;
        }
        self.overlay = Overlay::StatusPick;
    }

    pub fn open_priority_pick(&mut self) {
        if self.selected.is_none() {
            self.status_msg = "nothing selected".into();
            return;
        }
        self.overlay = Overlay::PriorityPick;
    }

    pub fn open_create(&mut self) {
        self.form = BeadForm::new(self.epics());
        self.overlay = Overlay::Create;
    }

    pub fn open_edit(&mut self) {
        let Some(bead) = self.selected_bead().cloned() else {
            self.status_msg = "nothing selected".into();
            return;
        };
        let mut form = BeadForm::new(self.epics());
        form.edit_id = Some(bead.id.clone());
        form.title = bead.title.clone();
        form.description = bead.description.clone();
        form.assignee = bead.assignee_raw().unwrap_or("").to_string();
        form.labels = bead.label_list().join(",");
        form.priority = bead.priority;
        form.type_index = ISSUE_TYPES
            .iter()
            .position(|kind| *kind == bead.issue_type)
            .unwrap_or(0);
        if let Some(parent) = bead.parent.as_deref()
            && let Some(index) = form.epics.iter().position(|(id, _)| id == parent)
        {
            form.epic_idx = index + 1;
        }
        self.form = form;
        self.overlay = Overlay::Create;
    }

    pub fn open_assign(&mut self) {
        if self.selected.is_none() {
            self.status_msg = "nothing selected".into();
            return;
        }
        self.refresh_panes();
        if self.live_panes.is_empty() {
            self.status_msg = "no live cmux panes. Launch from cmux or set CMUX_TUI_SOCKET.".into();
            return;
        }
        self.assign_index = self
            .live_panes
            .iter()
            .position(|pane| pane.active)
            .unwrap_or(0);
        self.overlay = Overlay::Assign;
    }

    pub fn confirm_assign(&mut self) {
        let Some(pane) = self.live_panes.get(self.assign_index).cloned() else {
            self.status_msg = "no live pane".into();
            return;
        };
        let Some(id) = self.selected.clone() else {
            return;
        };
        let assignee = pane.assignee();
        let label = pane.label();
        self.overlay = Overlay::None;
        self.write(
            format!("assign {id} -> {label} ({assignee})"),
            |cwd, scope| bd::set_assignee(cwd, scope, &id, &assignee),
        );
    }

    /// Back out one overlay. Never quits.
    pub fn escape(&mut self) {
        match self.overlay {
            Overlay::None => {
                if self.move_mode {
                    self.move_mode = false;
                    self.status_msg = "move mode off".into();
                } else if !self.filter.is_empty() {
                    self.filter.clear();
                    self.status_msg = "filter cleared".into();
                }
            }
            Overlay::Filter
            | Overlay::CloseReason
            | Overlay::Note
            | Overlay::Comment
            | Overlay::StatusPick
            | Overlay::PriorityPick
            | Overlay::Create
            | Overlay::Assign => {
                self.overlay = Overlay::None;
                self.prompt.clear();
            }
            Overlay::Help | Overlay::Detail => self.overlay = Overlay::None,
        }
    }

    pub fn submit_prompt(&mut self) {
        match self.overlay {
            Overlay::Filter => {
                self.filter = self.prompt.clone();
                self.overlay = Overlay::None;
                self.prompt.clear();
                self.selected = self.visible().first().map(|bead| bead.id.clone());
            }
            Overlay::CloseReason => {
                let reason = self.prompt.trim().to_string();
                if reason.is_empty() {
                    self.status_msg = "close needs a reason".into();
                    return;
                }
                let Some(id) = self.selected.clone() else {
                    return;
                };
                self.overlay = Overlay::None;
                self.prompt.clear();
                self.write(format!("close {id}"), |cwd, scope| {
                    bd::close(cwd, scope, &id, &reason)
                });
            }
            Overlay::Note => {
                let text = self.prompt.trim().to_string();
                if text.is_empty() {
                    self.status_msg = "note is empty".into();
                    return;
                }
                let Some(id) = self.selected.clone() else {
                    return;
                };
                self.overlay = Overlay::None;
                self.prompt.clear();
                self.write(format!("note {id}"), |cwd, scope| {
                    bd::add_note(cwd, scope, &id, &text)
                });
            }
            Overlay::Comment => {
                let text = self.prompt.trim().to_string();
                if text.is_empty() {
                    self.status_msg = "comment is empty".into();
                    return;
                }
                let Some(id) = self.selected.clone() else {
                    return;
                };
                self.overlay = Overlay::None;
                self.prompt.clear();
                self.write(format!("comment {id}"), |cwd, scope| {
                    bd::add_comment(cwd, scope, &id, &text)
                });
            }
            _ => {}
        }
    }

    pub fn prompt_push(&mut self, ch: char) {
        if self.overlay == Overlay::Create {
            self.form.input_char(ch);
            return;
        }
        self.prompt.push(ch);
    }

    pub fn prompt_backspace(&mut self) {
        if self.overlay == Overlay::Create {
            self.form.backspace();
            return;
        }
        self.prompt.pop();
    }

    pub fn submit_form(&mut self) {
        let title = self.form.title.trim().to_string();
        if title.is_empty() {
            self.status_msg = "title is required".into();
            return;
        }
        let issue_type = self.form.issue_type().to_string();
        let description = self.form.description.clone();
        let assignee = self.form.assignee.clone();
        let parent = self.form.parent_id().to_string();
        let labels = self.form.labels.clone();
        let priority = self.form.priority;
        let deferred = self.form.deferred;
        let edit_id = self.form.edit_id.clone();
        self.overlay = Overlay::None;
        let new_bead = NewBead {
            title: &title,
            issue_type: &issue_type,
            priority,
            description: &description,
            assignee: &assignee,
            parent: &parent,
            labels: &labels,
            deferred,
        };
        if let Some(id) = edit_id {
            match bd::update_bead(&self.cwd, self.scope, &id, &new_bead) {
                Ok(()) => {
                    self.status_msg = format!("updated {id}");
                    self.selected = Some(id);
                    self.reload();
                }
                Err(err) => self.status_msg = format!("update failed: {err}"),
            }
        } else {
            match bd::create(&self.cwd, self.scope, &new_bead) {
                Ok(id) => {
                    self.status_msg = if id.is_empty() {
                        format!("created {title}")
                    } else {
                        format!("created {id}")
                    };
                    if !id.is_empty() {
                        self.selected = Some(id.lines().next().unwrap_or(&id).trim().to_string());
                    }
                    self.reload();
                }
                Err(err) => self.status_msg = format!("create failed: {err}"),
            }
        }
    }

    pub fn claim_selected(&mut self) {
        let Some(bead) = self.selected_bead() else {
            self.status_msg = "nothing selected".into();
            return;
        };
        let id = bead.id.clone();
        self.write(format!("claim {id}"), |cwd, scope| {
            bd::claim(cwd, scope, &id)
        });
    }

    pub fn pick_status(&mut self, index: usize) {
        self.overlay = Overlay::None;
        let choices = self.statuses();
        let Some(status) = choices.get(index).cloned() else {
            self.status_msg = "no such status".into();
            return;
        };
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.write(format!("status {id} -> {status}"), |cwd, scope| {
            bd::set_status(cwd, scope, &id, &status)
        });
    }

    pub fn pick_priority(&mut self, priority: u8) {
        self.overlay = Overlay::None;
        if priority > 4 {
            self.status_msg = "priority must be 0-4".into();
            return;
        }
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.write(format!("priority {id} -> {priority}"), |cwd, scope| {
            bd::set_priority(cwd, scope, &id, priority)
        });
    }

    pub fn toggle_ready(&mut self) {
        self.ready_only = !self.ready_only;
        self.status_msg = if self.ready_only {
            "filter: ready".into()
        } else {
            "filter: all".into()
        };
        self.reload();
    }

    pub fn toggle_closed(&mut self) {
        self.include_closed = !self.include_closed;
        self.status_msg = if self.include_closed {
            "showing closed".into()
        } else {
            "hiding closed".into()
        };
        self.reload();
    }

    pub fn toggle_scope(&mut self) {
        let previous = self.scope;
        self.scope = self.scope.toggled();
        match bd::load(&self.cwd, self.scope, ListMode::All, self.include_closed) {
            Ok(beads) => {
                self.beads = beads;
                self.status_msg = format!("scope: {}", self.scope.label());
                self.last_refresh = Instant::now();
            }
            Err(err) => {
                self.scope = previous;
                self.status_msg = format!(
                    "scope {} failed ({err}); staying on {}",
                    self.scope.toggled().label(),
                    previous.label()
                );
            }
        }
    }

    pub fn cycle_sort(&mut self) {
        if self.view != BoardView::Table {
            return;
        }
        self.sort = self.sort.next();
        self.status_msg = format!("sort: {}", self.sort.label());
    }

    pub fn toggle_move_mode(&mut self) {
        self.move_mode = !self.move_mode;
        self.status_msg = if self.move_mode {
            "move mode: h/l retags status".into()
        } else {
            "move mode off".into()
        };
    }

    fn write(
        &mut self,
        label: String,
        op: impl FnOnce(&std::path::Path, Scope) -> Result<(), BridgeError>,
    ) {
        match op(&self.cwd, self.scope) {
            Ok(()) => {
                self.status_msg = format!("{label} · cwd {}", self.cwd.display());
                self.reload();
            }
            Err(err) => {
                self.status_msg = format!("{label} failed: {err} · cwd {}", self.cwd.display());
            }
        }
    }

    /// Plain-text board for `--selftest` (no TUI, no socket required).
    #[must_use]
    pub fn selftest_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "cwd {} ({})\nscope {}\nview {}\nmode {}\n",
            self.cwd.display(),
            self.cwd_source,
            self.scope.label(),
            self.view.title(),
            if self.ready_only { "ready" } else { "list" }
        ));
        if matches!(self.health, Health::BdMissing) {
            out.push_str("bd missing\n");
            return out;
        }
        match self.view {
            BoardView::Kanban => {
                for (status, cards) in self.kanban() {
                    out.push_str(&format!("\n[{status}]\n"));
                    for bead in cards {
                        out.push_str(&format!(
                            "  P{} {} {}\n",
                            bead.priority, bead.id, bead.title
                        ));
                    }
                }
            }
            BoardView::List | BoardView::Table => {
                let mut last_status = String::new();
                for bead in self.visible() {
                    if bead.status != last_status {
                        out.push_str(&format!("\n[{}]\n", bead.status));
                        last_status = bead.status.clone();
                    }
                    out.push_str(&format!(
                        "  P{} {} {}\n",
                        bead.priority, bead.id, bead.title
                    ));
                }
            }
        }
        if self.beads.is_empty() {
            out.push_str("(empty)\n");
        }
        if !self.live_workspaces.is_empty() {
            out.push_str("\nlive workspaces\n");
            for workspace in &self.live_workspaces {
                let flag = if workspace.active { "*" } else { " " };
                out.push_str(&format!(
                    "{flag} #{} {} ({} panes)\n",
                    workspace.id, workspace.name, workspace.pane_count
                ));
            }
        }
        if !self.live_panes.is_empty() {
            out.push_str("\nlive panes\n");
            for pane in &self.live_panes {
                out.push_str(&format!("  {} = {}\n", pane.assignee(), pane.label()));
            }
        }
        out
    }
}

fn jump_index(current: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    if delta == i32::MIN / 2 {
        0
    } else if delta == i32::MAX / 2 {
        len - 1
    } else {
        current.saturating_add_signed(delta as isize).min(len - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bd::parse_list;

    fn seeded() -> App {
        let mut app = App::new(Some(PathBuf::from("/tmp")));
        app.beads = parse_list(include_str!("../tests/fixtures/list.json")).unwrap();
        app.selected = Some("demo-1".into());
        app.health = Health::Standalone;
        app.view = BoardView::List;
        app
    }

    #[test]
    fn nav_wraps_inside_visible_rows() {
        let mut app = seeded();
        app.nav_vert(1);
        assert_eq!(app.selected.as_deref(), Some("demo-2"));
        app.nav_vert(i32::MAX / 2);
        assert_eq!(app.selected.as_deref(), Some("demo-3"));
        app.nav_vert(i32::MIN / 2);
        assert_eq!(app.selected.as_deref(), Some("demo-1"));
    }

    #[test]
    fn escape_clears_filter_and_never_quits() {
        let mut app = seeded();
        app.filter = "dark".into();
        app.escape();
        assert!(app.filter.is_empty());
        assert!(!app.should_quit);
        app.overlay = Overlay::Help;
        app.escape();
        assert_eq!(app.overlay, Overlay::None);
        assert!(!app.should_quit);
    }

    #[test]
    fn filter_overlay_narrows_visible() {
        let mut app = seeded();
        app.overlay = Overlay::Filter;
        app.prompt = "flaky".into();
        let rows = app.visible();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "demo-3");
    }

    #[test]
    fn selftest_text_groups_fixture_statuses() {
        let app = seeded();
        let text = app.selftest_text();
        assert!(text.contains("[open]"));
        assert!(text.contains("demo-1"));
        assert!(text.contains("[in_progress]"));
        assert!(text.contains("view List"));
    }

    #[test]
    fn kanban_h_moves_selection_across_columns() {
        let mut app = seeded();
        app.view = BoardView::Kanban;
        app.selected = Some("demo-1".into());
        app.nav_horiz(1);
        assert_eq!(app.selected.as_deref(), Some("demo-2"));
    }

    #[test]
    fn cycle_view_is_in_process() {
        let mut app = seeded();
        app.cycle_view(false);
        assert_eq!(app.view, BoardView::Table);
        app.cycle_view(false);
        assert_eq!(app.view, BoardView::Kanban);
        app.cycle_view(true);
        assert_eq!(app.view, BoardView::Table);
    }

    #[test]
    fn live_summary_is_empty_until_the_socket_tree_arrives() {
        let mut app = seeded();
        assert!(app.live_summary().is_empty());
        app.live_workspaces = vec![crate::sessions::LiveWorkspace {
            id: 1,
            name: "alpha".into(),
            active: true,
            pane_count: 2,
        }];
        app.live_panes = vec![crate::sessions::LivePane {
            workspace_id: 1,
            workspace_name: "alpha".into(),
            screen_id: 10,
            screen_name: "editor".into(),
            pane_id: 12,
            pane_title: "agent".into(),
            active: true,
        }];
        assert_eq!(app.live_summary(), "1ws 1pn");
        let text = app.selftest_text();
        assert!(text.contains("live workspaces"));
        assert!(text.contains("#1 alpha"));
        assert!(text.contains("cmux:1/12"));
    }
}
