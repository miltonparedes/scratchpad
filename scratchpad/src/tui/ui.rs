use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::models::Context;

use super::app::{App, Focus, Mode};

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(size);

    let content_area = main_chunks[0];
    let status_area = main_chunks[1];

    if app.show_preview {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(content_area);

        draw_session_list(f, app, chunks[0]);
        draw_notes_panel(f, app, chunks[1]);
    } else {
        draw_session_list(f, app, content_area);
    }

    draw_status_bar(f, app, status_area);

    match app.mode {
        Mode::Search => draw_input_popup(f, app, "Search", size),
        Mode::NewSession => draw_input_popup(f, app, "New Session (name, Enter for random)", size),
        Mode::QuickSession => draw_input_popup(f, app, "Quick Session (note)", size),
        Mode::Help => draw_help_popup(f, size),
        Mode::Normal => {}
    }

    if let Some(ref err) = app.error_message {
        draw_error_popup(f, err, size);
    }
}

fn draw_session_list(f: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.focus == Focus::List && app.mode == Mode::Normal {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = app
        .filtered_sessions
        .iter()
        .enumerate()
        .filter_map(|(i, &idx)| {
            app.sessions.get(idx).map(|session| {
                let style = if i == app.selected_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let date = session.updated_at.format("%m/%d %H:%M");
                let content = Line::from(vec![
                    Span::styled(&session.slug, style),
                    Span::styled(format!("  {date}"), Style::default().fg(Color::DarkGray)),
                ]);

                ListItem::new(content).style(style)
            })
        })
        .collect();

    let context_label = match &app.context {
        Context::User => "User".to_string(),
        Context::Project(_) => format!("Project: {}", app.context.display_name()),
    };

    let title = if app.search_query.is_empty() {
        format!(" {context_label} ({}) ", app.filtered_sessions.len())
    } else {
        format!(
            " {context_label} ({}/{}) [{}] ",
            app.filtered_sessions.len(),
            app.sessions.len(),
            app.search_query
        )
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(list, area);
}

fn draw_notes_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let border_style = if app.focus == Focus::Detail && app.mode == Mode::Normal {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = app
        .selected_session()
        .map(|s| format!(" {} ", s.display_title()))
        .unwrap_or_else(|| " Notes ".to_string());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let show_tree = app.file_tree.len() > 1;

    if show_tree {
        let tree_content_height = app.file_tree.len() as u16 + 2;
        let max_tree = (inner_area.height * 40 / 100).min(12);
        let tree_height = tree_content_height.min(max_tree).min(inner_area.height);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(tree_height), Constraint::Min(1)])
            .split(inner_area);

        let tree_area = chunks[0];
        let content_area = chunks[1];

        let tree_text = render_file_tree(&app.file_tree, tree_area.width);
        let tree_widget = Paragraph::new(tree_text);
        f.render_widget(tree_widget, tree_area);

        let content_text = build_content_text(app, content_area);
        let content_widget = Paragraph::new(content_text)
            .wrap(Wrap { trim: false })
            .scroll((app.notes_scroll, 0));
        f.render_widget(content_widget, content_area);
    } else {
        let content_text = build_content_text(app, inner_area);
        let content_widget = Paragraph::new(content_text)
            .wrap(Wrap { trim: false })
            .scroll((app.notes_scroll, 0));
        f.render_widget(content_widget, inner_area);
    }
}

fn build_content_text(app: &mut App, area: Rect) -> Text<'static> {
    if !app.session_files.is_empty() {
        let mut lines = vec![Line::from(Span::styled(
            "No markdown entry point. Files:",
            Style::default().fg(Color::Yellow),
        ))];
        lines.push(Line::from(""));

        for file in &app.session_files {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file.display().to_string());
            lines.push(Line::from(format!("  {name}")));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press 'e' to create notes.md, 'o' to open folder",
            Style::default().fg(Color::DarkGray),
        )));

        Text::from(lines)
    } else if app.notes_content.is_empty() {
        Text::from(Line::from(Span::styled(
            "(empty)",
            Style::default().fg(Color::DarkGray),
        )))
    } else {
        let content_width = area.width.max(20);
        app.ensure_rendered_notes(content_width);
        app.rendered_notes
            .clone()
            .unwrap_or_else(|| Text::from(Line::from("(render failed)")))
    }
}

fn render_file_tree(tree: &[crate::models::FileTreeEntry], _width: u16) -> Text<'static> {
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        format!("  Files ({})", tree.len()),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )));

    for entry in tree {
        let mut spans = Vec::new();

        spans.push(Span::raw("  "));
        for &ancestor_last in &entry.ancestor_is_last {
            if ancestor_last {
                spans.push(Span::raw("    "));
            } else {
                spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
                spans.push(Span::raw("   "));
            }
        }

        let connector = if entry.is_last {
            "└── "
        } else {
            "├── "
        };
        spans.push(Span::styled(
            connector,
            Style::default().fg(Color::DarkGray),
        ));

        let color = file_type_color(&entry.name, entry.is_dir);
        let mut style = Style::default().fg(color);
        if entry.is_entry_point {
            style = style.add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(entry.name.clone(), style));

        if entry.is_entry_point {
            spans.push(Span::styled("  ●", Style::default().fg(Color::Cyan)));
        }

        lines.push(Line::from(spans));
    }

    lines.push(Line::from(Span::styled(
        "─".repeat(20),
        Style::default().fg(Color::DarkGray),
    )));

    Text::from(lines)
}

fn file_type_color(name: &str, is_dir: bool) -> Color {
    if is_dir {
        return Color::Blue;
    }
    match name.rsplit('.').next() {
        Some("md") => Color::Cyan,
        Some("rs" | "py" | "js" | "ts" | "go" | "rb" | "c" | "cpp" | "h" | "java" | "sh") => {
            Color::Green
        }
        Some("toml" | "json" | "yaml" | "yml" | "xml" | "ini" | "env") => Color::Yellow,
        Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico") => Color::Magenta,
        Some("log") => Color::DarkGray,
        _ => Color::White,
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mode_str = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Search => "SEARCH",
        Mode::NewSession => "NEW",
        Mode::QuickSession => "QUICK",
        Mode::Help => "HELP",
    };

    let keybinds = match app.mode {
        Mode::Normal => {
            if app.available_contexts.len() > 1 {
                "n:new Q:quick /:search r:run e:edit v:view o:folder g:context ?:help q:quit"
            } else {
                "n:new Q:quick /:search r:run e:edit v:view o:folder ?:help q:quit"
            }
        }
        Mode::Search | Mode::NewSession | Mode::QuickSession => "Enter:confirm Esc:cancel",
        Mode::Help => "Esc/q:close",
    };

    let status = Line::from(vec![
        Span::styled(
            format!(" {mode_str} "),
            Style::default().bg(Color::Cyan).fg(Color::Black),
        ),
        Span::raw(" "),
        Span::styled(keybinds, Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(status);
    f.render_widget(paragraph, area);
}

fn draw_input_popup(f: &mut Frame, app: &App, title: &str, area: Rect) {
    let popup_area = centered_rect_fixed_height(60, 3, area);
    f.render_widget(Clear, popup_area);

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} "))
                .border_style(Style::default().fg(Color::Yellow)),
        );

    f.render_widget(input, popup_area);

    f.set_cursor_position((popup_area.x + app.input.len() as u16 + 1, popup_area.y + 1));
}

fn draw_help_popup(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(55, 70, area);
    f.render_widget(Clear, popup_area);

    let help_text = Text::from(vec![
        Line::from(Span::styled(
            "ScratchPad Keybindings",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("n", Style::default().fg(Color::Cyan)),
            Span::raw("        New session (name or auto-generate)"),
        ]),
        Line::from(vec![
            Span::styled("Q", Style::default().fg(Color::Cyan)),
            Span::raw("        Quick session (with note)"),
        ]),
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw("        Search sessions"),
        ]),
        Line::from(vec![
            Span::styled("r", Style::default().fg(Color::Cyan)),
            Span::raw("        Run agent in session"),
        ]),
        Line::from(vec![
            Span::styled("e", Style::default().fg(Color::Cyan)),
            Span::raw("        Edit notes in $EDITOR"),
        ]),
        Line::from(vec![
            Span::styled("v", Style::default().fg(Color::Cyan)),
            Span::raw("        View notes in viewer"),
        ]),
        Line::from(vec![
            Span::styled("o", Style::default().fg(Color::Cyan)),
            Span::raw("        Open session folder"),
        ]),
        Line::from(vec![
            Span::styled("g", Style::default().fg(Color::Cyan)),
            Span::raw("        Toggle context (User/Project)"),
        ]),
        Line::from(vec![
            Span::styled("p", Style::default().fg(Color::Cyan)),
            Span::raw("        Toggle preview panel"),
        ]),
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Cyan)),
            Span::raw("      Switch focus"),
        ]),
        Line::from(vec![
            Span::styled("j/k", Style::default().fg(Color::Cyan)),
            Span::raw("      Navigate up/down"),
        ]),
        Line::from(vec![
            Span::styled("PgUp/Dn", Style::default().fg(Color::Cyan)),
            Span::raw("  Scroll notes"),
        ]),
        Line::from(vec![
            Span::styled("Esc", Style::default().fg(Color::Cyan)),
            Span::raw("      Clear search / Cancel"),
        ]),
        Line::from(vec![
            Span::styled("?", Style::default().fg(Color::Cyan)),
            Span::raw("        Show this help"),
        ]),
        Line::from(vec![
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::raw("        Quit"),
        ]),
    ]);

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(help, popup_area);
}

fn draw_error_popup(f: &mut Frame, message: &str, area: Rect) {
    let popup_area = centered_rect_fixed_height(60, 3, area);
    f.render_widget(Clear, popup_area);

    let error = Paragraph::new(message).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Error ")
            .border_style(Style::default().fg(Color::Red)),
    );

    f.render_widget(error, popup_area);
}

fn centered_rect_fixed_height(percent_x: u16, height: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(height),
            Constraint::Fill(1),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
