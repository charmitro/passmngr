//! Terminal UI rendering

use passmngr::app::{App, Mode};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Truncate string to fit within max_width, adding "..." if truncated
fn truncate_string(s: &str, max_width: usize) -> String {
    let char_count = s.chars().count();
    let needs_truncate = char_count > max_width;

    if needs_truncate {
        if max_width <= 3 {
            ".".repeat(max_width)
        } else {
            let truncated: String = s.chars().take(max_width - 3).collect();
            format!("{}...", truncated)
        }
    } else {
        format!("{:<width$}", s, width = max_width)
    }
}

/// Render the application UI
pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Search bar
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Footer/help
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_search_bar(f, app, chunks[1]);
    render_main_content(f, app, chunks[2]);
    render_footer(f, app, chunks[3]);
}

/// Render the header with app name and status
fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let mode_text = format!("[{}]", app.mode.as_str());
    let count_text = format!("{} entries", app.filtered_entries.len());
    let status_text = if app.dirty { "modified" } else { "saved" };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "passmngr ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            &mode_text,
            Style::default()
                .fg(match app.mode {
                    Mode::Normal => Color::Green,
                    Mode::Insert => Color::Yellow,
                    Mode::Search => Color::Blue,
                    Mode::Command => Color::Magenta,
                    Mode::Detail => Color::Cyan,
                    Mode::Locked => Color::Red,
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::styled(&count_text, Style::default().fg(Color::Gray)),
        Span::raw(" | "),
        Span::styled(
            status_text,
            Style::default().fg(if app.dirty {
                Color::Yellow
            } else {
                Color::Green
            }),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL))
    .alignment(Alignment::Left);

    f.render_widget(header, area);
}

/// Render the search bar
fn render_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let search_text = if app.mode == Mode::Search {
        format!("Search: {}_", app.search_query)
    } else if !app.search_query.is_empty() {
        format!("Search: {}", app.search_query)
    } else {
        "Search: ".to_string()
    };

    let search_bar = Paragraph::new(search_text)
        .style(Style::default().fg(if app.mode == Mode::Search {
            Color::Yellow
        } else {
            Color::Gray
        }))
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(search_bar, area);
}

/// Render the main content area
fn render_main_content(f: &mut Frame, app: &mut App, area: Rect) {
    match app.mode {
        Mode::Insert => render_form_view(f, app, area),
        Mode::Detail => render_detail_view(f, app, area),
        Mode::Locked => render_locked_view(f, app, area),
        _ => render_list_view(f, app, area),
    }
}

/// Render the locked view
fn render_locked_view(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Red));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(3),
            Constraint::Percentage(40),
        ])
        .split(inner_area);

    let title = Paragraph::new(vec![
        Line::from(Span::styled(
            "🔒 VAULT LOCKED",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("Session timed out due to inactivity."),
    ])
    .alignment(Alignment::Center);

    f.render_widget(title, chunks[0]);

    let input = Paragraph::new(format!("Password: {}", "*".repeat(app.unlock_input.len())))
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL).title("Unlock"));

    f.render_widget(input, chunks[1]);
}

/// Render the list of entries
fn render_list_view(f: &mut Frame, app: &mut App, area: Rect) {
    // Calculate column widths based on available terminal width
    // Account for: borders (2), indicator (2), spacing (2)
    let available_width = area.width.saturating_sub(6) as usize;

    // Distribute width: name gets 40%, username gets 35%, tags get 25%
    // But ensure minimum widths
    let name_width = (available_width * 40 / 100).clamp(15, 40);
    let username_width = (available_width * 35 / 100).clamp(15, 35);
    let tags_width = available_width.saturating_sub(name_width + username_width);

    let items: Vec<ListItem> = app
        .filtered_entries
        .iter()
        .enumerate()
        .filter_map(|(i, id)| {
            let entry = app.vault.get_entry(id)?;
            let is_selected = i == app.selected;

            let tags_str = if entry.tags.is_empty() {
                String::new()
            } else {
                format!("[{}]", entry.tags.join(", "))
            };

            let name_display = truncate_string(&entry.name, name_width);
            let username_display = truncate_string(&entry.username, username_width);
            let tags_display = truncate_string(&tags_str, tags_width);

            let line = Line::from(vec![
                Span::styled(
                    if is_selected { "> " } else { "  " },
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    name_display,
                    Style::default().fg(if is_selected {
                        Color::White
                    } else {
                        Color::Gray
                    }),
                ),
                Span::raw(" "),
                Span::styled(username_display, Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::styled(tags_display, Style::default().fg(Color::Blue)),
            ]);

            Some(ListItem::new(line).style(if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            }))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, area, &mut app.list_state);
}

/// Render detailed view of selected entry
fn render_detail_view(f: &mut Frame, app: &App, area: Rect) {
    let entry = match app.get_selected_entry() {
        Some(e) => e,
        None => {
            let text = Text::from("No entry selected");
            let paragraph = Paragraph::new(text).block(Block::default().borders(Borders::ALL));
            f.render_widget(paragraph, area);
            return;
        }
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Cyan)),
            Span::raw(&entry.name),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Username: ", Style::default().fg(Color::Cyan)),
            Span::raw(&entry.username),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Password: ", Style::default().fg(Color::Cyan)),
            if app.show_password {
                Span::raw(&entry.password)
            } else {
                Span::raw("*".repeat(entry.password.len()))
            },
        ]),
        Line::from(""),
    ];

    if let Some(url) = &entry.url {
        lines.push(Line::from(vec![
            Span::styled("URL: ", Style::default().fg(Color::Cyan)),
            Span::raw(url),
        ]));
        lines.push(Line::from(""));
    }

    if !entry.tags.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Tags: ", Style::default().fg(Color::Cyan)),
            Span::raw(entry.tags.join(", ")),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(notes) = &entry.notes {
        lines.push(Line::from(vec![Span::styled(
            "Notes: ",
            Style::default().fg(Color::Cyan),
        )]));
        lines.push(Line::from(notes.as_str()));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Created: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            entry.created.format("%Y-%m-%d %H:%M:%S").to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Modified: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            entry.modified.format("%Y-%m-%d %H:%M:%S").to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Entry Details"),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Render the form for creating/editing entries
fn render_form_view(f: &mut Frame, app: &App, area: Rect) {
    use passmngr::app::FormField;

    let title = if app.form_data.editing_id.is_some() {
        "Edit Entry"
    } else {
        "New Entry"
    };

    let fields = [
        FormField::Name,
        FormField::Username,
        FormField::Password,
        FormField::Url,
        FormField::Notes,
        FormField::Tags,
    ];

    let mut lines = vec![Line::from("")];

    for field in fields.iter() {
        let is_focused = &app.focused_field == field;
        let label = field.as_str();
        let value = app.get_field_value(*field);

        let display_value =
            if field == &FormField::Password && !value.is_empty() && !app.show_password {
                "*".repeat(value.len())
            } else {
                value.to_string()
            };

        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<12} ", label),
                Style::default().fg(if is_focused {
                    Color::Yellow
                } else {
                    Color::Cyan
                }),
            ),
            Span::styled(
                if is_focused {
                    format!("{}_", display_value)
                } else {
                    display_value
                },
                Style::default().fg(if is_focused {
                    Color::White
                } else {
                    Color::Gray
                }),
            ),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Tab/Shift+Tab:", Style::default().fg(Color::Green)),
        Span::raw(" Next/Prev field  "),
        Span::styled("Ctrl+S:", Style::default().fg(Color::Green)),
        Span::raw(" Save  "),
        Span::styled("Ctrl+P:", Style::default().fg(Color::Green)),
        Span::raw(" Show/Hide  "),
        Span::styled("Ctrl+G:", Style::default().fg(Color::Green)),
        Span::raw(" Generate  "),
        Span::styled("Esc:", Style::default().fg(Color::Green)),
        Span::raw(" Cancel"),
    ]));

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// Render the footer with help text or command buffer
fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let content = match app.mode {
        Mode::Locked => Line::from(vec![
            Span::styled("Enter:", Style::default().fg(Color::White)),
            Span::raw(" Unlock  "),
            Span::styled("Esc/q:", Style::default().fg(Color::White)),
            Span::raw(" Quit"),
        ]),
        Mode::Command => {
            let mut spans = vec![
                Span::styled(":", Style::default().fg(Color::Magenta)),
                Span::raw(app.command_buffer.clone()),
                Span::raw("_"),
            ];

            if app.command_completions.len() > 1 {
                // Show available completions
                spans.push(Span::raw("  "));
                spans.push(Span::styled("Tab: ", Style::default().fg(Color::Green)));
                spans.push(Span::styled(
                    app.command_completions.join(" | "),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        }
        Mode::Detail => Line::from(vec![
            Span::styled("Esc/q:", Style::default().fg(Color::Green)),
            Span::raw("back  "),
            Span::styled("e:", Style::default().fg(Color::Green)),
            Span::raw("edit  "),
            Span::styled("y/Y:", Style::default().fg(Color::Green)),
            Span::raw("copy pass/user  "),
            Span::styled("v:", Style::default().fg(Color::Green)),
            Span::raw("show/hide"),
        ]),
        _ => {
            if let Some(status) = &app.status_message {
                Line::from(Span::styled(
                    status,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled("j/k:", Style::default().fg(Color::Green)),
                    Span::raw("nav  "),
                    Span::styled("/:", Style::default().fg(Color::Green)),
                    Span::raw("search  "),
                    Span::styled("n:", Style::default().fg(Color::Green)),
                    Span::raw("new  "),
                    Span::styled("e:", Style::default().fg(Color::Green)),
                    Span::raw("edit  "),
                    Span::styled("d:", Style::default().fg(Color::Green)),
                    Span::raw("delete  "),
                    Span::styled("y:", Style::default().fg(Color::Green)),
                    Span::raw("copy-pass  "),
                    Span::styled(":q:", Style::default().fg(Color::Green)),
                    Span::raw("quit"),
                ])
            }
        }
    };

    let footer = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);

    f.render_widget(footer, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello     ");
        assert_eq!(truncate_string("hello world", 5), "he...");
        assert_eq!(truncate_string("hello", 3), "...");
    }

    #[test]
    fn test_truncate_string_multibyte() {
        // Each emoji is multiple bytes
        let s = "😀😃😄😁😆";
        // 5 chars.
        // If max width is 10, it should pad.
        assert_eq!(truncate_string(s, 10), "😀😃😄😁😆     ");

        // If max width is 4, it should truncate to 1 char + ...
        // 4 - 3 = 1 char.
        assert_eq!(truncate_string(s, 4), "😀...");

        // Test the specific panic case: slicing in middle of multibyte
        // If we used byte slicing:
        // s is ~20 bytes. max_width 7. 7-3 = 4 bytes.
        // First emoji is 4 bytes. So it would work by luck if emoji is exactly 4 bytes.
        // But let's use a char that is 3 bytes. "你" (U+4F60) is 3 bytes (E4 BD A0).
        let s2 = "你好世界!";
        // 5 chars. 15+1 bytes.
        // max_width 4. 5 > 4 so truncate.
        // keep 4-3 = 1 char.
        assert_eq!(truncate_string(s2, 4), "你...");
    }
}
