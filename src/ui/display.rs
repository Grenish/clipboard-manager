use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::clipboard::{ClipboardBackend, set_clipboard_image, set_clipboard_text};
use crate::history::ClipboardHistory;
use crate::models::ClipboardContentType;
use crate::ui::app::AppState;

use std::time::Duration;

// ============================================================================
// TERMINAL UI DISPLAY
// ============================================================================

pub fn show_ui(backend: ClipboardBackend) -> Result<(), Box<dyn std::error::Error>> {
    let history = ClipboardHistory::new();

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend_term = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend_term)?;
    terminal.clear()?;

    let mut app_state = AppState::new();

    loop {
        let entries = history.get_all();

        terminal.draw(|f| {
            if app_state.show_clear_confirm {
                // Clear confirmation dialog
                let area = f.area();
                let text = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "⚠️  Clear All History?",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "This will permanently delete all clipboard entries and images.",
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press Y to confirm • N or Esc to cancel",
                        Style::default().fg(Color::Gray),
                    )),
                ])
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red)),
                );

                let centered = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(35),
                        Constraint::Length(9),
                        Constraint::Percentage(35),
                    ])
                    .split(area);

                let h_centered = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(20),
                        Constraint::Percentage(60),
                        Constraint::Percentage(20),
                    ])
                    .split(centered[1]);

                f.render_widget(text, h_centered[1]);
            } else if entries.is_empty() {
                // Empty state
                let area = f.area();
                let text = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Clipboard History Empty",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Copy text or images to start",
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press Esc to close",
                        Style::default().fg(Color::Gray),
                    )),
                ])
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                );

                let centered = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(40),
                        Constraint::Length(9),
                        Constraint::Percentage(40),
                    ])
                    .split(area);

                f.render_widget(text, centered[1]);
            } else {
                // Main UI
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(2)])
                    .split(f.area());

                let items: Vec<ListItem> = entries
                    .iter()
                    .map(|entry| {
                        let color = match entry.content_type {
                            ClipboardContentType::Text => Color::White,
                            ClipboardContentType::Image => Color::Cyan,
                        };

                        ListItem::new(Line::from(vec![
                            Span::styled(format!(" {} ", entry.icon()), Style::default().fg(color)),
                            Span::styled(entry.display_content(), Style::default().fg(color)),
                            Span::styled(
                                format!(" {}", entry.formatted_time()),
                                Style::default().fg(Color::Gray),
                            ),
                        ]))
                    })
                    .collect();

                let list = List::new(items)
                    .block(
                        Block::default()
                            .title(format!(" Clipboard ({}) ", entries.len()))
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Cyan)),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▶ ");

                f.render_stateful_widget(list, chunks[0], &mut app_state.list_state);
                let footer =
                    Paragraph::new("↑↓: Navigate  │  Enter: Copy  │  D: Delete Selected  │  C: Clear All  │  Esc: Close")
                        .style(Style::default().fg(Color::Gray))
                        .alignment(Alignment::Center);

                f.render_widget(footer, chunks[1]);
            }
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let CrosstermEvent::Key(KeyEvent { code, .. }) = event::read()? {
                if app_state.show_clear_confirm {
                    match code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            history.clear();
                            app_state.show_clear_confirm = false;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app_state.show_clear_confirm = false;
                        }
                        _ => {}
                    }
                } else {
                    let entries_len = entries.len();
                    match code {
                        KeyCode::Char('q') | KeyCode::Esc => app_state.quit(),
                        KeyCode::Char('c') | KeyCode::Char('C') if entries_len > 0 => {
                            app_state.show_clear_confirm = true;
                        }
                        KeyCode::Down | KeyCode::Char('j') => app_state.next(entries_len),
                        KeyCode::Up | KeyCode::Char('k') => app_state.previous(entries_len),
                        KeyCode::Enter if entries_len > 0 => app_state.select(),
                        KeyCode::Char('d') | KeyCode::Delete if entries_len > 0 => {
                            if let Some(index) = app_state.list_state.selected() {
                                history.delete_entry(index);
                                // Adjust selection if we deleted the last item
                                if index >= history.get_all().len() && index > 0 {
                                    app_state.list_state.select(Some(index - 1));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if app_state.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Some(index) = app_state.selected_index {
        let entries = history.get_all();
        if let Some(entry) = entries.get(index) {
            match entry.content_type {
                ClipboardContentType::Text => {
                    if set_clipboard_text(&entry.content, backend).is_ok() {
                        println!("✓ Copied to clipboard");
                    }
                }
                ClipboardContentType::Image => {
                    let image_path = history.images_dir().join(&entry.content);
                    if set_clipboard_image(&image_path, backend).is_ok() {
                        println!("✓ Copied image to clipboard");
                    }
                }
            }
        }
    }

    Ok(())
}