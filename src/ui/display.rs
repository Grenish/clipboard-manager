use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode},
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
        // Filter entries based on search query
        let all_entries = history.get_all();
        let filtered_entries: Vec<&crate::models::ClipboardEntry> = if app_state.is_searching && !app_state.search_query.is_empty() {
            all_entries.iter().filter(|e| {
                e.content.to_lowercase().contains(&app_state.search_query.to_lowercase())
            }).collect()
        } else {
            all_entries.iter().collect()
        };

        terminal.draw(|f| {
            if app_state.show_clear_confirm {
                 // ... (Clear confirm logic remains same, just rendering 'text' widget)
                 // Re-using existing logic requires minimal changes to structure.
                 // Ideally I should copy the block from lines 44-69 but since Iam replacing the whole loop body logic effectively...
                 // Let's stick to replacing the render logic.
                 
                let area = f.area();
                let text = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "⚠  Clear All History?",
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

            } else if all_entries.is_empty() { // Check ORIGINAL list for empty
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

                let header_title = if app_state.is_searching {
                    Paragraph::new(Span::styled(
                        format!(" 🔍 Search: {}_", app_state.search_query),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ))
                } else {
                    Paragraph::new(Span::styled(
                        " 📋 Clipboard",
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    ))
                };
                f.render_widget(header_title, header_chunks[0]);

                let current_idx = if filtered_entries.is_empty() { 0 } else { app_state.list_state.selected().unwrap_or(0) + 1 };
                let total_count = filtered_entries.len();
                let max_history = crate::utils::MAX_HISTORY;
                
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
                let list_inner_width = chunks[1].width.saturating_sub(4) as usize; 

                let items: Vec<ListItem> = filtered_entries
                    .iter()
                    .map(|entry| {
                        let mut lines = vec![];
                        
                        let preview = entry.preview_lines();
                        for line in preview {
                            lines.push(Line::from(format!(" {}", line)));
                        }
                        
                        let meta = entry.metadata_label();
                        let paddable_width = list_inner_width.saturating_sub(1);
                        let aligned_meta = format!("{:>width$}", meta, width = paddable_width);

                        lines.push(Line::from(Span::styled(
                            aligned_meta, 
                            Style::default().fg(Color::DarkGray)
                        )));
                        
                        lines.push(Line::from(""));

                        ListItem::new(lines)
                    })
                    .collect();
                
                // Show "No Results" if searching and empty
                let list = if items.is_empty() && app_state.is_searching {
                     List::new(vec![ListItem::new("No matches found")])
                        .block(
                             Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Rounded)
                                .border_style(Style::default().fg(Color::Cyan))
                        )
                        .style(Style::default().fg(Color::Red))
                } else {
                    List::new(items)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Rounded)
                                .border_style(Style::default().fg(Color::Cyan))
                        )
                        .style(Style::default().fg(Color::Gray))
                        .highlight_style(
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        )
                        .highlight_symbol("▍ ")
                };

                f.render_stateful_widget(list, chunks[1], &mut app_state.list_state);

                // ========================
                // 3. FOOTER (Styled Keys)
                // ========================
                let key_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
                let text_style = Style::default().fg(Color::White);
                let sep_style = Style::default().fg(Color::DarkGray);

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
            if let CrosstermEvent::Key(key) = event::read()? {
                if app_state.show_clear_confirm {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            history.clear();
                            app_state.show_clear_confirm = false;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app_state.show_clear_confirm = false;
                        }
                        _ => {}
                    }
                } else if app_state.is_searching {
                    // Search Mode Input Handling
                    match key.code {
                        KeyCode::Esc => {
                            app_state.is_searching = false;
                            app_state.search_query.clear();
                        }
                        KeyCode::Enter => {
                             // Confirm selection
                             app_state.select();
                        }
                        KeyCode::Char(c) => {
                            app_state.search_query.push(c);
                            // Reset selection to top on search change
                            app_state.list_state.select(Some(0));
                        }
                        KeyCode::Backspace => {
                            app_state.search_query.pop();
                            app_state.list_state.select(Some(0));
                        }
                        KeyCode::Down => app_state.next(filtered_entries.len()),
                        KeyCode::Up => app_state.previous(filtered_entries.len()),
                         _ => {}
                    }
                } else {
                    // Normal Mode Input Handling
                    let entries_len = filtered_entries.len(); // Use filtered length!
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app_state.quit(),
                        KeyCode::Char('c') | KeyCode::Char('C') if entries_len > 0 => {
                            app_state.show_clear_confirm = true;
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') => { // Enter Search Mode
                            app_state.is_searching = true;
                            app_state.search_query.clear();
                            app_state.list_state.select(Some(0));
                        }
                        KeyCode::Down | KeyCode::Char('j') => app_state.next(entries_len),
                        KeyCode::Up | KeyCode::Char('k') => app_state.previous(entries_len),
                        KeyCode::Enter if entries_len > 0 => app_state.select(),
                        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete if entries_len > 0 => {
                            if let Some(index) = app_state.list_state.selected() {
                                // Need to map filtered index back to original history index?
                                // Ah, deleting while filtering is tricky. 
                                // Simplest way: if filtering, you can't delete? Or we need to find the real entry.
                                // For now, let's disable delete on search or handle it carefully.
                                // If I delete index '0' of filtered list, which item is it in real list?
                                // I need to get the actual entry from filtered_entries[index], find its hash/id, and delete that.
                                // Since I don't have IDs easily exposted to 'delete_entry(index)', I might have to skip delete in search for MVP or standard index delete.
                                // Actually, 'filtered_entries' is Vec<&Entry>. I can't easily map back index unless I search.
                                // Let's disable delete in search mode for now to avoid accidental deletions of wrong items.
                                if !app_state.is_searching { 
                                     history.delete_entry(index);
                                     let new_len = history.get_all().len();
                                     if new_len == 0 {
                                         app_state.list_state.select(None);
                                     } else if index >= new_len {
                                         app_state.list_state.select(Some(index - 1));
                                     }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        


        if app_state.should_quit {
            // Capture selected entry before exiting if we were selecting
            if let Some(idx) = app_state.list_state.selected() {
                 if let Some(entry) = filtered_entries.get(idx) {
                     // Only set if we actually "Selected" (pressed enter)
                     // But wait, 'should_quit' is also for ESC.
                     // We need to know if it was a selection.
                     // 'select()' sets selected_index.
                     if app_state.selected_index.is_some() {
                         app_state.selected_entry = Some((*entry).clone());
                     }
                 }
            }
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Use captured entry instead of index lookup
    if let Some(entry) = app_state.selected_entry {
        let mut pasted = false;
        match entry.content_type {
            ClipboardContentType::Text => {
                if set_clipboard_text(&entry.content, backend).is_ok() {
                    println!("✓ Copied to clipboard");
                    pasted = true;
                }
            }
            ClipboardContentType::Image => {
                let image_path = history.images_dir().join(&entry.content);
                if set_clipboard_image(&image_path, backend).is_ok() {
                    println!("✓ Copied image to clipboard");
                    pasted = true;
                }
            }
        }

        if pasted {
           // Spawn a detached process to handle pasting after the UI closes
           // This prevents the clipboard manager window from receiving the simulated keys
           if let Ok(exe) = std::env::current_exe() {
               std::process::Command::new(exe)
                   .arg("--paste")
                   .spawn()
                   .ok();
           }
        }
    }

    Ok(())
}
