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
    widgets::{Block, Borders, BorderType, List, ListItem, Paragraph},
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
                // Main UI Layout
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // Header Text
                        Constraint::Min(0),    // List (Boxed)
                        Constraint::Length(1), // Footer Text
                    ])
                    .split(f.area());

                // ========================
                // 1. HEADER (Styled)
                // ========================
                let header_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(50), // Title
                        Constraint::Percentage(50), // Stats
                    ])
                    .split(chunks[0]);

                let header_title = Paragraph::new(Span::styled(
                    " 📋 Clipboard",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ));
                f.render_widget(header_title, header_chunks[0]);

                let current_idx = app_state.list_state.selected().unwrap_or(0) + 1;
                let total_count = entries.len();
                let max_history = crate::utils::MAX_HISTORY;
                
                // Style: "1/5" (White) " | " (DarkGray) "50" (DarkGray)
                let stats_spans = vec![
                    Span::styled(
                        format!("{}/{}", current_idx, total_count), 
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    ),
                    Span::styled(
                        format!(" | max {}", max_history), 
                        Style::default().fg(Color::DarkGray)
                    ),
                ];

                let header_stats = Paragraph::new(Line::from(stats_spans))
                    .alignment(Alignment::Right);
                f.render_widget(header_stats, header_chunks[1]);

                // ========================
                // 2. LIST (Themed)
                // ========================
                
                // Calculate valid width for content inside list
                let list_inner_width = chunks[1].width.saturating_sub(4) as usize; 

                let items: Vec<ListItem> = entries
                    .iter()
                    .map(|entry| {
                        let mut lines = vec![];
                        
                        // Content Preview 
                        // We rely on the List's style for the text color (Gray), 
                        // and Highlight style for selected (White).
                        // So we don't hardcode Color::White here anymore.
                        let preview = entry.preview_lines();
                        for line in preview {
                            lines.push(Line::from(format!(" {}", line)));
                        }
                        
                        // Metadata (Right Aligned)
                        let meta = entry.metadata_label();
                        let paddable_width = list_inner_width.saturating_sub(1);
                        
                        let aligned_meta = format!("{:>width$}", meta, width = paddable_width);

                        lines.push(Line::from(Span::styled(
                            aligned_meta, 
                            Style::default().fg(Color::DarkGray) // Metadata stays dim
                        )));
                        
                        // Add spacing
                        lines.push(Line::from(""));

                        ListItem::new(lines)
                    })
                    .collect();

                let list = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::Cyan)) // Accent Border
                    )
                    .style(Style::default().fg(Color::Gray)) // Default "Comfy" Text
                    .highlight_style(
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▍ ");

                f.render_stateful_widget(list, chunks[1], &mut app_state.list_state);

                // ========================
                // 3. FOOTER (Styled Keys)
                // ========================
                // Format: <Key> Action | <Key> Action
                let key_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
                let text_style = Style::default().fg(Color::White); // Brighter text
                let sep_style = Style::default().fg(Color::DarkGray); // Visible separator

                let footer_spans = vec![
                    Span::styled("↑↓", key_style), Span::styled(" Nav ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" Enter", key_style), Span::styled(" Copy ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" D", key_style), Span::styled(" Del ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" S", key_style), Span::styled(" Search ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" C", key_style), Span::styled(" Clear ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" Esc", key_style), Span::styled(" Close", text_style),
                ];

                let footer = Paragraph::new(Line::from(footer_spans))
                    .alignment(Alignment::Center);

                f.render_widget(footer, chunks[2]);
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
                        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete if entries_len > 0 => {
                            if let Some(index) = app_state.list_state.selected() {
                                history.delete_entry(index);
                                // Adjust selection if we deleted the last item
                                let new_len = history.get_all().len();
                                if new_len == 0 {
                                    app_state.list_state.select(None);
                                } else if index >= new_len {
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
