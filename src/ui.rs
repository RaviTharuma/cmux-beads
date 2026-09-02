//! Ratatui views for the cmux sidebar PTY.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

use crate::app::{App, Health, Overlay};
use crate::bd::ISSUE_TYPES;
use crate::board::BoardView;
use crate::form::{F_ASSIGNEE, F_BACKLOG, F_DESC, F_EPIC, F_LABELS, F_PRIORITY, F_TITLE, F_TYPE};
use crate::keys;
use crate::sessions;

const ACCENT: Color = Color::Rgb(137, 180, 250);
const MUTED: Color = Color::Rgb(108, 112, 134);
const TEXT: Color = Color::Rgb(205, 214, 244);
const WARN: Color = Color::Rgb(249, 226, 175);
const ERR: Color = Color::Rgb(243, 139, 168);
const OK: Color = Color::Rgb(166, 227, 161);

/// Draw the current overlay + board into the sidebar frame.
pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if matches!(app.health, Health::BdMissing) {
        draw_missing_bd(frame, area, app);
        return;
    }

    let detail_height = if app.show_detail_pane { 8 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(detail_height),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(frame, chunks[0], app);
    match app.view {
        BoardView::List => draw_list(frame, chunks[1], app),
        BoardView::Table => draw_table(frame, chunks[1], app),
        BoardView::Kanban => draw_kanban(frame, chunks[1], app),
    }
    if app.show_detail_pane {
        draw_detail_inline(frame, chunks[2], app);
    }
    draw_footer(frame, chunks[3], app);

    match app.overlay {
        Overlay::Help => draw_help(frame, area),
        Overlay::Detail => draw_detail_modal(frame, area, app),
        Overlay::Filter => draw_prompt(frame, area, "filter", &app.prompt),
        Overlay::CloseReason => draw_prompt(frame, area, "close reason", &app.prompt),
        Overlay::Note => draw_prompt(frame, area, "note", &app.prompt),
        Overlay::Comment => draw_prompt(frame, area, "comment", &app.prompt),
        Overlay::StatusPick => draw_status_pick(frame, area, app),
        Overlay::PriorityPick => draw_priority_pick(frame, area),
        Overlay::Create => draw_form(frame, area, app),
        Overlay::Assign => draw_assign(frame, area, app),
        Overlay::None => {}
    }
}

fn draw_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let cwd = truncate_middle(
        &app.cwd.display().to_string(),
        area.width.saturating_sub(8) as usize,
    );
    let mode = if app.ready_only { "ready" } else { "all" };
    let health = match &app.health {
        Health::Ready => Span::styled("cmux", Style::default().fg(OK)),
        Health::Standalone => Span::styled("solo", Style::default().fg(WARN)),
        Health::Reconnecting { .. } => Span::styled("wait", Style::default().fg(WARN)),
        Health::BdMissing => Span::styled("no bd", Style::default().fg(ERR)),
    };
    let filter = if app.filter.is_empty() {
        String::new()
    } else {
        format!(" /{}", app.filter)
    };
    let move_flag = if app.move_mode { " move" } else { "" };
    let line = Line::from(vec![
        Span::styled(
            "cmux-beads ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        health,
        Span::styled(
            format!(
                " {} {} {}{filter}{move_flag}",
                app.view.title(),
                app.scope.label(),
                mode
            ),
            Style::default().fg(MUTED),
        ),
    ]);
    let cwd_line = Line::from(Span::styled(
        format!("{} {cwd}", app.cwd_source),
        Style::default().fg(MUTED),
    ));
    frame.render_widget(Paragraph::new(vec![line, cwd_line]), area);
}

fn draw_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = app.visible();
    if rows.is_empty() {
        draw_empty(frame, area, app);
        return;
    }
    let mut items = Vec::new();
    let mut last_status = String::new();
    for bead in &rows {
        if bead.status != last_status {
            items.push(ListItem::new(Line::from(Span::styled(
                format!(" {}", bead.status.replace('_', " ")),
                Style::default()
                    .fg(status_color(&bead.status))
                    .add_modifier(Modifier::BOLD),
            ))));
            last_status = bead.status.clone();
        }
        items.push(bead_row(app, bead, area.width));
    }
    frame.render_widget(List::new(items), area);
}

fn draw_table(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = app.visible();
    if rows.is_empty() {
        draw_empty(frame, area, app);
        return;
    }
    let mut items = vec![ListItem::new(Line::from(Span::styled(
        format!(" P  {:<10} {:<10} title", "id", "status"),
        Style::default().fg(MUTED),
    )))];
    for bead in &rows {
        items.push(bead_row(app, bead, area.width));
    }
    frame.render_widget(List::new(items), area);
}

fn draw_kanban(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = app.kanban();
    if columns.iter().all(|(_, cards)| cards.is_empty()) && app.visible().is_empty() {
        draw_empty(frame, area, app);
        return;
    }
    let current_status = app
        .selected_bead()
        .map(|bead| bead.status.clone())
        .or_else(|| columns.first().map(|(status, _)| status.clone()))
        .unwrap_or_else(|| "open".into());
    let wide = area.width >= 60 && columns.len() >= 2;
    if wide {
        let widths: Vec<Constraint> = columns
            .iter()
            .map(|_| Constraint::Percentage((100 / columns.len().max(1) as u16).max(1)))
            .collect();
        let cells = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(widths)
            .split(area);
        for (index, (status, cards)) in columns.iter().enumerate() {
            draw_kanban_column(
                frame,
                cells[index],
                app,
                status,
                cards,
                status == &current_status,
            );
        }
    } else {
        let Some((_, cards)) = columns
            .iter()
            .find(|(status, _)| status == &current_status)
            .or(columns.first())
        else {
            draw_empty(frame, area, app);
            return;
        };
        let header = format!(
            "← {} →  {} cards",
            current_status.replace('_', " "),
            cards.len()
        );
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        frame.render_widget(
            Paragraph::new(header).style(
                Style::default()
                    .fg(status_color(&current_status))
                    .add_modifier(Modifier::BOLD),
            ),
            chunks[0],
        );
        let items: Vec<ListItem> = cards
            .iter()
            .map(|bead| bead_row(app, bead, chunks[1].width))
            .collect();
        frame.render_widget(List::new(items), chunks[1]);
    }
}

fn draw_kanban_column(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    status: &str,
    cards: &[&crate::bd::Bead],
    focused: bool,
) {
    let title = format!(" {} ({}) ", status.replace('_', " "), cards.len());
    let border = if focused { ACCENT } else { MUTED };
    let items: Vec<ListItem> = cards
        .iter()
        .map(|bead| bead_row(app, bead, area.width.saturating_sub(2)))
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        ),
        area,
    );
}

fn bead_row(app: &App, bead: &crate::bd::Bead, width: u16) -> ListItem<'static> {
    let selected = app.selected.as_deref() == Some(bead.id.as_str());
    let marker = if selected { "▸" } else { " " };
    let style = if selected {
        Style::default().fg(TEXT).bg(Color::Rgb(49, 50, 68))
    } else {
        Style::default().fg(TEXT)
    };
    let assigned = sessions::parse_assignee(bead.assignee_display()).is_some();
    let flag = if assigned { "*" } else { " " };
    let title = truncate_middle(
        &bead.title,
        width.saturating_sub(bead.id.len() as u16 + 8) as usize,
    );
    ListItem::new(Line::from(vec![
        Span::styled(
            format!("{marker}P{}", bead.priority),
            Style::default().fg(WARN),
        ),
        Span::styled(format!(" {}", bead.id), Style::default().fg(ACCENT)),
        Span::styled(flag.to_string(), Style::default().fg(OK)),
        Span::styled(format!(" {title}"), style),
    ]))
}

fn draw_empty(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let msg = if !app.filter.is_empty() {
        "no issues match filter"
    } else if app.ready_only {
        "nothing ready. R for all, a to create."
    } else {
        "no issues. a create · r refresh"
    };
    frame.render_widget(
        Paragraph::new(msg)
            .style(Style::default().fg(MUTED))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let count = app.visible().len();
    let total = app.beads.len();
    let keys = "K views  A assign  ? help";
    let msg = if app.status_msg.is_empty() {
        format!("{count}/{total}  {keys}")
    } else {
        truncate_middle(&app.status_msg, area.width.saturating_sub(1) as usize)
    };
    let style = if app.status_msg.contains("failed") || app.status_msg.contains("not on PATH") {
        Style::default().fg(ERR)
    } else {
        Style::default().fg(MUTED)
    };
    frame.render_widget(
        Paragraph::new(msg).style(style).wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_missing_bd(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let body = vec![
        Line::from(Span::styled(
            "bd is not on PATH",
            Style::default().fg(ERR).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Install beads v0.60+"),
        Line::from(Span::styled(
            "https://github.com/steveyegge/beads",
            Style::default().fg(ACCENT),
        )),
        Line::from(""),
        Line::from("brew install beads"),
        Line::from("or npm i -g @beads/bd"),
        Line::from(""),
        Line::from("The mux stays up."),
        Line::from("Press r to retry, q to quit."),
        Line::from(""),
        Line::from(Span::styled(
            app.status_msg.clone(),
            Style::default().fg(MUTED),
        )),
    ];
    frame.render_widget(
        Paragraph::new(body)
            .block(
                Block::default()
                    .title(" cmux-beads ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ERR)),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_help(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered(area, 94, 90);
    frame.render_widget(Clear, popup);
    let lines: Vec<Line> = keys::help_lines()
        .into_iter()
        .map(|(key, action)| {
            Line::from(vec![
                Span::styled(format!("{key:<16}"), Style::default().fg(ACCENT)),
                Span::styled(action, Style::default().fg(TEXT)),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" keys ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT)),
            )
            .wrap(Wrap { trim: true }),
        popup,
    );
}

fn detail_lines(app: &App) -> Vec<Line<'static>> {
    let Some(bead) = app.detail.as_ref().or(app.selected_bead()) else {
        return vec![Line::from(Span::styled(
            "nothing selected",
            Style::default().fg(MUTED),
        ))];
    };
    let mut lines = vec![
        Line::from(Span::styled(
            bead.id.clone(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(bead.title.clone()),
        Line::from(""),
        Line::from(format!(
            "{}  P{}  {}",
            bead.status,
            bead.priority,
            if bead.issue_type.is_empty() {
                "task"
            } else {
                &bead.issue_type
            }
        )),
        Line::from(format!("assignee {}", bead.assignee_display())),
    ];
    if let Some(pane) = sessions::resolve_assignee(bead.assignee_display(), &app.live_panes) {
        lines.push(Line::from(Span::styled(
            format!("pane {}", pane.label()),
            Style::default().fg(OK),
        )));
    }
    if !bead.label_list().is_empty() {
        lines.push(Line::from(format!(
            "labels {}",
            bead.label_list().join(", ")
        )));
    }
    if let Some(parent) = bead.parent.as_deref() {
        lines.push(Line::from(format!("parent {parent}")));
    }
    lines.push(Line::from(""));
    if bead.description.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no description)",
            Style::default().fg(MUTED),
        )));
    } else {
        for paragraph in bead.description.lines() {
            lines.push(Line::from(paragraph.to_string()));
        }
    }
    if !bead.dependencies.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "dependencies",
            Style::default().fg(WARN),
        )));
        for dep in &bead.dependencies {
            lines.push(Line::from(dep.label()));
        }
    }
    if !bead.note_lines().is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("notes", Style::default().fg(WARN))));
        for note in bead.note_lines() {
            lines.push(Line::from(note));
        }
    }
    if !bead.comment_lines().is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "comments",
            Style::default().fg(WARN),
        )));
        for comment in bead.comment_lines() {
            lines.push(Line::from(comment));
        }
    }
    lines
}

fn draw_detail_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered(area, 96, 90);
    frame.render_widget(Clear, popup);
    let mut lines = detail_lines(app);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Esc back",
        Style::default().fg(MUTED),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" detail · bd show ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT)),
            )
            .wrap(Wrap { trim: true }),
        popup,
    );
}

fn draw_detail_inline(frame: &mut Frame<'_>, area: Rect, app: &App) {
    frame.render_widget(
        Paragraph::new(detail_lines(app))
            .block(
                Block::default()
                    .title(" detail ")
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(MUTED)),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_prompt(frame: &mut Frame<'_>, area: Rect, title: &str, value: &str) {
    let popup = Rect {
        x: area.x,
        y: area.bottom().saturating_sub(3),
        width: area.width,
        height: 3.min(area.height),
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(format!("{value}█")).block(
            Block::default()
                .title(format!(" {title} "))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        ),
        popup,
    );
}

fn draw_status_pick(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered(area, 80, 70);
    frame.render_widget(Clear, popup);
    let lines: Vec<Line> = app
        .statuses()
        .into_iter()
        .enumerate()
        .map(|(index, status)| Line::from(format!("  {}  {status}", index + 1)))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" status · 1-9 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        ),
        popup,
    );
}

fn draw_priority_pick(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered(area, 70, 40);
    frame.render_widget(Clear, popup);
    let lines = vec![
        Line::from("  0  highest"),
        Line::from("  1"),
        Line::from("  2  default"),
        Line::from("  3"),
        Line::from("  4  lowest"),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" priority · 0-4 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        ),
        popup,
    );
}

fn draw_form(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered(area, 96, 80);
    frame.render_widget(Clear, popup);
    let mark = |field: u8| {
        if app.form.field == field { ">" } else { " " }
    };
    let title = if app.form.edit_id.is_some() {
        " bd update "
    } else {
        " bd create "
    };
    let lines = vec![
        Line::from(format!(
            "{} type     {}  ← →",
            mark(F_TYPE),
            app.form.issue_type()
        )),
        Line::from(format!(
            "{} priority {}  ← →",
            mark(F_PRIORITY),
            app.form.priority
        )),
        Line::from(format!("{} title    {}█", mark(F_TITLE), app.form.title)),
        Line::from(format!(
            "{} desc     {}",
            mark(F_DESC),
            app.form.description
        )),
        Line::from(format!(
            "{} assignee {}",
            mark(F_ASSIGNEE),
            app.form.assignee
        )),
        Line::from(format!(
            "{} epic     {}",
            mark(F_EPIC),
            app.form.epic_label()
        )),
        Line::from(format!("{} labels   {}", mark(F_LABELS), app.form.labels)),
        Line::from(format!(
            "{} backlog  {}  (space)",
            mark(F_BACKLOG),
            if app.form.deferred { "yes" } else { "no" }
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("types: {}", ISSUE_TYPES.join(" ")),
            Style::default().fg(MUTED),
        )),
        Line::from(Span::styled(
            "Tab field · Enter save · Esc cancel",
            Style::default().fg(MUTED),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        ),
        popup,
    );
}

fn draw_assign(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered(area, 96, 80);
    frame.render_widget(Clear, popup);
    let mut lines = vec![Line::from(Span::styled(
        "assignee = cmux:{workspace_id}/{pane_id}",
        Style::default().fg(MUTED),
    ))];
    if app.live_panes.is_empty() {
        lines.push(Line::from("no live panes"));
    } else {
        for (index, pane) in app.live_panes.iter().enumerate() {
            let marker = if index == app.assign_index {
                "▸"
            } else {
                " "
            };
            lines.push(Line::from(format!(
                "{marker} {}  {}",
                pane.assignee(),
                pane.label()
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Enter assign · Esc cancel",
        Style::default().fg(MUTED),
    )));
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" assign to live cmux pane ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        ),
        popup,
    );
}

fn status_color(status: &str) -> Color {
    match status {
        "open" => OK,
        "in_progress" => ACCENT,
        "blocked" => ERR,
        "deferred" => MUTED,
        "closed" => MUTED,
        "pinned" => WARN,
        "hooked" => Color::Rgb(203, 166, 247),
        _ => TEXT,
    }
}

fn centered(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(popup)[1]
}

fn truncate_middle(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let head = max / 2;
    let tail = max.saturating_sub(head + 1);
    format!(
        "{}…{}",
        chars[..head].iter().collect::<String>(),
        chars[chars.len() - tail..].iter().collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_middle_keeps_short_strings() {
        assert_eq!(truncate_middle("abc", 8), "abc");
        assert_eq!(truncate_middle("abcdefghij", 7).chars().count(), 7);
        assert!(truncate_middle("abcdefghij", 7).contains('…'));
    }
}
