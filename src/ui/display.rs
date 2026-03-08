use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph},
};

use crate::clipboard::{ClipboardBackend, set_clipboard_image, set_clipboard_text};
use crate::history::ClipboardHistory;
use crate::models::ClipboardContentType;
use crate::ui::app::AppState;
use crate::ui::emoji;

use std::time::Duration;

// ============================================================================
// EMOJI GRID RENDERER
// ============================================================================

/// A single cell in the emoji grid (value + display name).
struct EmojiCell {
    value: String,
    name: String,
}

/// A span of text in the category tab strip, with its character-offset range.
struct TabSpan {
    text: String,
    start: usize, // inclusive char offset
    end: usize,   // exclusive char offset
}

/// Render a grid of emoji cells inside `grid_area`.
///
/// Instead of taking `&mut AppState` (which conflicts with the draw closure's
/// borrow), we accept the individual mutable fields we need to read/write.
fn render_emoji_grid(
    f: &mut ratatui::Frame,
    cells: &[EmojiCell],
    grid_area: Rect,
    item_index: &mut usize,
    grid_cols: &mut usize,
    grid_scroll: &mut usize,
) {
    // Inner area after border (1 cell each side)
    let inner_w = grid_area.width.saturating_sub(2) as usize;
    let inner_h = grid_area.height.saturating_sub(2) as usize;

    if cells.is_empty() || inner_w == 0 || inner_h == 0 {
        let empty = Paragraph::new(Span::styled(
            "  No matches found",
            Style::default().fg(Color::Red),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Magenta)),
        );
        f.render_widget(empty, grid_area);
        return;
    }

    // Determine column width: each cell shows the emoticon value + (name).
    // Pick a fixed cell width and compute how many columns fit.
    let cell_width: usize = 24;
    let cols = (inner_w / cell_width).max(1);
    *grid_cols = cols;

    // Clamp selection
    let total = cells.len();
    if *item_index >= total {
        *item_index = total - 1;
    }

    let total_rows = (total + cols - 1) / cols;

    // Ensure viewport follows the selected row
    {
        let current_row = *item_index / cols;
        if current_row < *grid_scroll {
            *grid_scroll = current_row;
        } else if inner_h > 0 && current_row >= *grid_scroll + inner_h {
            *grid_scroll = current_row - inner_h + 1;
        }
    }

    let scroll = *grid_scroll;
    let sel = *item_index;

    // Build visible lines
    let mut lines: Vec<Line> = Vec::with_capacity(inner_h);
    for row in scroll..(scroll + inner_h).min(total_rows) {
        let mut spans: Vec<Span> = Vec::new();
        for col in 0..cols {
            let idx = row * cols + col;
            if idx >= total {
                break;
            }
            let cell = &cells[idx];
            let is_sel = idx == sel;

            // Format: " value (name) " padded to cell_width
            let content = format!(" {} ({})", cell.value, cell.name);
            // Pad or truncate to cell_width
            let display: String = if content.chars().count() >= cell_width {
                content.chars().take(cell_width - 1).collect::<String>() + "…"
            } else {
                let pad = cell_width - content.chars().count();
                format!("{}{}", content, " ".repeat(pad))
            };

            let style = if is_sel {
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::Gray)
            };
            spans.push(Span::styled(display, style));
        }
        lines.push(Line::from(spans));
    }

    // Scrollbar hint in title
    let title_suffix = if total_rows > inner_h {
        let sel_row = sel / cols;
        format!(" {}/{} ", sel_row + 1, total_rows)
    } else {
        String::new()
    };

    let grid_widget = Paragraph::new(lines).block(
        Block::default()
            .title_bottom(
                Line::from(Span::styled(
                    title_suffix,
                    Style::default().fg(Color::DarkGray),
                ))
                .alignment(Alignment::Right),
            )
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta)),
    );
    f.render_widget(grid_widget, grid_area);
}

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

    // Build emoji categories once outside the loop
    let emoji_cats = emoji::categories();

    loop {
        // Clean up any expired secrets each tick
        history.cleanup_expired();

        // Filter entries based on search query
        let all_entries = history.get_all();
        let filtered_entries: Vec<&crate::models::ClipboardEntry> =
            if app_state.is_searching && !app_state.search_query.is_empty() {
                all_entries
                    .iter()
                    .filter(|e| {
                        let query = app_state.search_query.to_lowercase();
                        let (_icon, category_label) = e.detect_category();
                        // Match against content OR category label OR "secret" keyword
                        e.content.to_lowercase().contains(&query)
                            || category_label.to_lowercase() == query
                            || (query == "secret" && e.is_secret())
                    })
                    .collect()
            } else {
                all_entries.iter().collect()
            };

        // Clear reveal if the selected index changed away from the revealed entry
        if let Some(reveal_idx) = app_state.reveal_index {
            let current_sel = app_state.list_state.selected().unwrap_or(usize::MAX);
            if current_sel != reveal_idx {
                app_state.reveal_index = None;
            }
        }

        terminal.draw(|f| {
            // Background UI
            if all_entries.is_empty() {
                // Check ORIGINAL list for empty
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
                        "Press E for Emoji Picker • Esc to close",
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
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ))
                } else {
                    Paragraph::new(Span::styled(
                        " 📋 Clipboard",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ))
                };
                f.render_widget(header_title, header_chunks[0]);

                let current_idx = if filtered_entries.is_empty() {
                    0
                } else {
                    app_state.list_state.selected().unwrap_or(0) + 1
                };
                let total_count = filtered_entries.len();
                let max_history = crate::utils::MAX_HISTORY;

                let stats_spans = vec![
                    Span::styled(
                        format!("{}/{}", current_idx, total_count),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" | max {}", max_history),
                        Style::default().fg(Color::DarkGray),
                    ),
                ];

                let header_stats =
                    Paragraph::new(Line::from(stats_spans)).alignment(Alignment::Right);
                f.render_widget(header_stats, header_chunks[1]);

                // ========================
                // 2. LIST (Themed)
                // ========================
                let list_inner_width = chunks[1].width.saturating_sub(4) as usize;

                let items: Vec<ListItem> = filtered_entries
                    .iter()
                    .enumerate()
                    .map(|(idx, entry)| {
                        let mut lines = vec![];

                        // Determine if this entry should be revealed
                        let is_revealed = app_state.reveal_index == Some(idx);
                        let preview = entry.preview_lines_with_reveal(is_revealed);
                        for line in preview {
                            lines.push(Line::from(format!(" {}", line)));
                        }

                        let meta = entry.metadata_label();
                        let paddable_width = list_inner_width.saturating_sub(1);
                        let aligned_meta = format!("{:>width$}", meta, width = paddable_width);

                        // Use a different color for secret metadata
                        let meta_color = if entry.is_secret() {
                            Color::Yellow
                        } else {
                            Color::DarkGray
                        };

                        lines.push(Line::from(Span::styled(
                            aligned_meta,
                            Style::default().fg(meta_color),
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
                                .border_style(Style::default().fg(Color::Cyan)),
                        )
                        .style(Style::default().fg(Color::Red))
                } else {
                    List::new(items)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Rounded)
                                .border_style(Style::default().fg(Color::Cyan)),
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
                let key_style = Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD);
                let text_style = Style::default().fg(Color::White);
                let sep_style = Style::default().fg(Color::DarkGray);

                // Check if the currently selected entry is a secret to show contextual hints
                let selected_is_secret = app_state
                    .list_state
                    .selected()
                    .and_then(|idx| filtered_entries.get(idx))
                    .map(|e| e.is_secret())
                    .unwrap_or(false);

                let mut footer_spans = vec![
                    Span::styled("↑↓", key_style),
                    Span::styled(" Nav ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" Enter", key_style),
                    Span::styled(" Copy ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" P", key_style),
                    Span::styled(" Pin ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" D", key_style),
                    Span::styled(" Del ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" S", key_style),
                    Span::styled(" Search ", text_style),
                    Span::styled("|", sep_style),
                    Span::styled(" E", key_style),
                    Span::styled(" Emoji ", text_style),
                ];

                if selected_is_secret {
                    footer_spans.push(Span::styled("|", sep_style));
                    footer_spans.push(Span::styled(" R", key_style));
                    footer_spans.push(Span::styled(" Reveal ", text_style));
                    footer_spans.push(Span::styled("|", sep_style));
                    footer_spans.push(Span::styled(" ⇧S", key_style));
                    footer_spans.push(Span::styled(" Keep ", text_style));
                }

                footer_spans.push(Span::styled("|", sep_style));
                footer_spans.push(Span::styled(" C", key_style));
                footer_spans.push(Span::styled(" Clear ", text_style));
                footer_spans.push(Span::styled("|", sep_style));
                footer_spans.push(Span::styled(" Esc", key_style));
                footer_spans.push(Span::styled(" Close", text_style));

                let footer = Paragraph::new(Line::from(footer_spans)).alignment(Alignment::Center);

                f.render_widget(footer, chunks[2]);
            }

            // ========================================
            // MODAL: Clear Confirm
            // ========================================
            if app_state.show_clear_confirm {
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

                f.render_widget(Clear, h_centered[1]);
                f.render_widget(text, h_centered[1]);
            }

            // ========================================
            // MODAL: Emoji Picker
            // ========================================
            if app_state.show_emoji_picker {
                let area = f.area();

                // Overlay layout: centered box taking most of the screen
                let v_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(5),
                        Constraint::Min(10),
                        Constraint::Percentage(5),
                    ])
                    .split(area);

                let h_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(10),
                        Constraint::Min(40),
                        Constraint::Percentage(10),
                    ])
                    .split(v_chunks[1]);

                let picker_area = h_chunks[1];

                // Clear the background
                f.render_widget(Clear, picker_area);

                // Split picker into: search bar, category tabs, item grid, footer
                let picker_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // Search bar
                        Constraint::Length(3), // Category tabs
                        Constraint::Min(3),    // Item grid
                        Constraint::Length(1), // Footer
                    ])
                    .split(picker_area);

                // -- Search bar --
                let search_text = if app_state.emoji_search.is_empty() {
                    Span::styled(
                        " 🔍 Type to search emoticons...",
                        Style::default().fg(Color::DarkGray),
                    )
                } else {
                    Span::styled(
                        format!(" 🔍 {}_", app_state.emoji_search),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                };
                let search_bar = Paragraph::new(search_text);
                f.render_widget(search_bar, picker_chunks[0]);

                // Determine if we're in search mode or browsing mode
                let is_emoji_searching = !app_state.emoji_search.is_empty();

                if is_emoji_searching {
                    // -- Search results mode --
                    let search_results =
                        emoji::search_emoticons(&emoji_cats, &app_state.emoji_search);

                    // Category tabs: show "Search Results" label
                    let tab_line = Line::from(Span::styled(
                        format!("  🔍 Search Results ({})  ", search_results.len()),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ));
                    let tabs = Paragraph::new(tab_line).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::Magenta)),
                    );
                    f.render_widget(tabs, picker_chunks[1]);

                    // Build cells from search results
                    let cells: Vec<EmojiCell> = search_results
                        .iter()
                        .map(|r| EmojiCell {
                            value: r.value.to_string(),
                            name: r.name.to_string(),
                        })
                        .collect();

                    render_emoji_grid(
                        f,
                        &cells,
                        picker_chunks[2],
                        &mut app_state.emoji_item_index,
                        &mut app_state.emoji_grid_cols,
                        &mut app_state.emoji_grid_scroll,
                    );
                } else {
                    // -- Browse mode --
                    let cat_count = emoji_cats.len();
                    let selected_cat = app_state
                        .emoji_category_index
                        .min(cat_count.saturating_sub(1));

                    // ---- Scrollable category tab strip ----
                    // We manually render category labels into a single
                    // Line, computing a character-level scroll offset so
                    // the selected tab is always visible.

                    let tab_inner_w = picker_chunks[1].width.saturating_sub(2) as usize; // border

                    // Build each tab string and record char-offset ranges
                    let mut tab_spans: Vec<TabSpan> = Vec::new();
                    let mut cursor: usize = 0;
                    for (idx, cat) in emoji_cats.iter().enumerate() {
                        let label = format!(" {} {} ", cat.icon, cat.name);
                        let len = label.chars().count();
                        // Add divider before all but the first
                        if idx > 0 {
                            let div = " │ ";
                            let div_len = div.chars().count();
                            tab_spans.push(TabSpan {
                                text: div.to_string(),
                                start: cursor,
                                end: cursor + div_len,
                            });
                            cursor += div_len;
                        }
                        tab_spans.push(TabSpan {
                            text: label,
                            start: cursor,
                            end: cursor + len,
                        });
                        cursor += len;
                    }

                    // Find the char range of the selected category's label
                    // (skip divider spans — category labels are at even-ish
                    // positions; simpler: find by index)
                    let mut sel_start: usize = 0;
                    let mut sel_end: usize = 0;
                    {
                        let mut cat_idx = 0;
                        for ts in &tab_spans {
                            if ts.text.starts_with(" │") || ts.text == " │ " {
                                continue; // divider
                            }
                            if cat_idx == selected_cat {
                                sel_start = ts.start;
                                sel_end = ts.end;
                                break;
                            }
                            cat_idx += 1;
                        }
                    }

                    // Compute scroll offset so selected tab is visible,
                    // with a small margin so context tabs show on each side.
                    let margin: usize = 3;
                    let mut scroll_off: usize = 0;
                    if sel_end + margin > tab_inner_w {
                        scroll_off = (sel_end + margin).saturating_sub(tab_inner_w);
                    }
                    if sel_start < scroll_off + margin {
                        scroll_off = sel_start.saturating_sub(margin);
                    }

                    // Build the visible spans by slicing the virtual line
                    let mut visible_spans: Vec<Span> = Vec::new();

                    // Arrow indicator if there are hidden tabs to the left
                    let has_left = scroll_off > 0;
                    let has_right = cursor > scroll_off + tab_inner_w;
                    let arrow_w: usize =
                        (if has_left { 2 } else { 0 }) + (if has_right { 2 } else { 0 });
                    let content_w = tab_inner_w.saturating_sub(arrow_w);

                    if has_left {
                        visible_spans.push(Span::styled(
                            "◀ ",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }

                    // Render tab spans clipped to the visible window
                    let vis_start = scroll_off;
                    let vis_end = scroll_off + content_w;
                    let mut cat_idx: usize = 0;
                    for ts in &tab_spans {
                        let is_divider = ts.text.starts_with(" │") || ts.text == " │ ";

                        // Skip spans entirely outside the window
                        if ts.end <= vis_start || ts.start >= vis_end {
                            if !is_divider {
                                cat_idx += 1;
                            }
                            continue;
                        }

                        // Compute how much of this span is visible
                        let clip_start = if ts.start < vis_start {
                            vis_start - ts.start
                        } else {
                            0
                        };
                        let clip_end_chars = ts.text.chars().count();
                        let avail = if ts.end > vis_end {
                            clip_end_chars - (ts.end - vis_end)
                        } else {
                            clip_end_chars
                        };
                        let visible_text: String = ts
                            .text
                            .chars()
                            .skip(clip_start)
                            .take(avail.saturating_sub(clip_start))
                            .collect();

                        if visible_text.is_empty() {
                            if !is_divider {
                                cat_idx += 1;
                            }
                            continue;
                        }

                        let style = if is_divider {
                            Style::default().fg(Color::DarkGray)
                        } else if cat_idx == selected_cat {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };

                        visible_spans.push(Span::styled(visible_text, style));

                        if !is_divider {
                            cat_idx += 1;
                        }
                    }

                    if has_right {
                        visible_spans.push(Span::styled(
                            " ▶",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }

                    let tab_bar = Paragraph::new(Line::from(visible_spans))
                        .alignment(Alignment::Left)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Rounded)
                                .border_style(Style::default().fg(Color::Magenta)),
                        );
                    f.render_widget(tab_bar, picker_chunks[1]);

                    // ---- Emoji grid for the selected category ----
                    let current_cat = selected_cat;
                    let cells: Vec<EmojiCell> = emoji_cats
                        .get(current_cat)
                        .map(|category| {
                            category
                                .emoticons
                                .iter()
                                .map(|e| EmojiCell {
                                    value: e.value.to_string(),
                                    name: e.name.to_string(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    render_emoji_grid(
                        f,
                        &cells,
                        picker_chunks[2],
                        &mut app_state.emoji_item_index,
                        &mut app_state.emoji_grid_cols,
                        &mut app_state.emoji_grid_scroll,
                    );
                }

                // -- Picker footer --
                let pk = Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD);
                let pt = Style::default().fg(Color::White);
                let ps = Style::default().fg(Color::DarkGray);

                let picker_footer_spans = if app_state.emoji_search.is_empty() {
                    vec![
                        Span::styled("↑↓←→", pk),
                        Span::styled(" Navigate ", pt),
                        Span::styled("|", ps),
                        Span::styled(" Tab/⇧Tab", pk),
                        Span::styled(" Category ", pt),
                        Span::styled("|", ps),
                        Span::styled(" Enter", pk),
                        Span::styled(" Copy ", pt),
                        Span::styled("|", ps),
                        Span::styled(" Type", pk),
                        Span::styled(" Search ", pt),
                        Span::styled("|", ps),
                        Span::styled(" Esc", pk),
                        Span::styled(" Close", pt),
                    ]
                } else {
                    vec![
                        Span::styled("↑↓←→", pk),
                        Span::styled(" Navigate ", pt),
                        Span::styled("|", ps),
                        Span::styled(" Enter", pk),
                        Span::styled(" Copy ", pt),
                        Span::styled("|", ps),
                        Span::styled(" Esc", pk),
                        Span::styled(" Clear search", pt),
                    ]
                };

                let picker_footer =
                    Paragraph::new(Line::from(picker_footer_spans)).alignment(Alignment::Center);
                f.render_widget(picker_footer, picker_chunks[3]);
            }
        })?;

        // ====================================================================
        // INPUT HANDLING
        // ====================================================================
        if event::poll(Duration::from_millis(50))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                // ---- Emoji Picker Mode ----
                if app_state.show_emoji_picker {
                    let is_emoji_searching = !app_state.emoji_search.is_empty();

                    // Compute the current total items for grid nav
                    let emoji_total = if is_emoji_searching {
                        emoji::search_emoticons(&emoji_cats, &app_state.emoji_search).len()
                    } else {
                        emoji::category_item_count(&emoji_cats, app_state.emoji_category_index)
                    };

                    match key.code {
                        KeyCode::Esc => {
                            if is_emoji_searching {
                                // Clear search first
                                app_state.emoji_search.clear();
                                app_state.emoji_item_index = 0;
                                app_state.emoji_grid_scroll = 0;
                            } else {
                                app_state.close_emoji_picker();
                            }
                        }
                        KeyCode::Enter => {
                            if is_emoji_searching {
                                let results =
                                    emoji::search_emoticons(&emoji_cats, &app_state.emoji_search);
                                if !results.is_empty() {
                                    let idx = app_state.emoji_item_index.min(results.len() - 1);
                                    app_state.emoji_selected = Some(results[idx].value.to_string());
                                    app_state.close_emoji_picker();
                                }
                            } else {
                                let cat = app_state.emoji_category_index;
                                let item_count = emoji::category_item_count(&emoji_cats, cat);
                                if item_count > 0 {
                                    let safe_index = app_state.emoji_item_index.min(item_count - 1);
                                    if let Some(value) =
                                        emoji::get_emoticon(&emoji_cats, cat, safe_index)
                                    {
                                        app_state.emoji_selected = Some(value.to_string());
                                        app_state.close_emoji_picker();
                                    }
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if !app_state.emoji_search.is_empty() {
                                app_state.emoji_search.pop();
                                app_state.emoji_item_index = 0;
                                app_state.emoji_grid_scroll = 0;
                            }
                        }
                        // Grid navigation: ↑↓ move rows, ←→ move cells
                        KeyCode::Down => {
                            app_state.emoji_grid_next_row(emoji_total);
                        }
                        KeyCode::Up => {
                            app_state.emoji_grid_prev_row(emoji_total);
                        }
                        KeyCode::Right => {
                            app_state.emoji_grid_next_col(emoji_total);
                        }
                        KeyCode::Left => {
                            app_state.emoji_grid_prev_col(emoji_total);
                        }
                        // Tab / Shift+Tab cycles categories (browse mode only)
                        KeyCode::Tab => {
                            if !is_emoji_searching {
                                app_state.emoji_next_category(emoji_cats.len());
                            }
                        }
                        KeyCode::BackTab => {
                            if !is_emoji_searching {
                                app_state.emoji_prev_category(emoji_cats.len());
                            }
                        }
                        KeyCode::Char(c) => {
                            app_state.emoji_search.push(c);
                            app_state.emoji_item_index = 0;
                            app_state.emoji_grid_scroll = 0;
                        }
                        _ => {}
                    }
                }
                // ---- Clear Confirm Mode ----
                else if app_state.show_clear_confirm {
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
                }
                // ---- Search Mode ----
                else if app_state.is_searching {
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
                }
                // ---- Normal Mode ----
                else {
                    let entries_len = filtered_entries.len();
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app_state.quit(),
                        KeyCode::Char('c') | KeyCode::Char('C') if entries_len > 0 => {
                            app_state.show_clear_confirm = true;
                        }
                        // Shift+S: stop expiry on a secret entry
                        KeyCode::Char('S')
                            if key.modifiers.contains(KeyModifiers::SHIFT) && entries_len > 0 =>
                        {
                            if let Some(index) = app_state.list_state.selected() {
                                if !app_state.is_searching {
                                    if let Some(entry) = filtered_entries.get(index) {
                                        if entry.is_secret() {
                                            history.stop_expiry(index);
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char('s') => {
                            // Enter Search Mode (lowercase s only)
                            app_state.is_searching = true;
                            app_state.search_query.clear();
                            app_state.list_state.select(Some(0));
                        }
                        // E: open emoji picker
                        KeyCode::Char('e') | KeyCode::Char('E') => {
                            app_state.open_emoji_picker();
                        }
                        KeyCode::Down | KeyCode::Char('j') => app_state.next(entries_len),
                        KeyCode::Up | KeyCode::Char('k') => app_state.previous(entries_len),
                        KeyCode::Enter if entries_len > 0 => app_state.select(),
                        // R: toggle reveal on a secret entry
                        KeyCode::Char('r') | KeyCode::Char('R') if entries_len > 0 => {
                            if let Some(index) = app_state.list_state.selected() {
                                if let Some(entry) = filtered_entries.get(index) {
                                    if entry.is_secret() {
                                        if app_state.reveal_index == Some(index) {
                                            // Toggle off
                                            app_state.reveal_index = None;
                                        } else {
                                            app_state.reveal_index = Some(index);
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char('p') | KeyCode::Char('P') if entries_len > 0 => {
                            if let Some(index) = app_state.list_state.selected() {
                                if !app_state.is_searching {
                                    history.toggle_pin(index);
                                }
                            }
                        }
                        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete
                            if entries_len > 0 =>
                        {
                            if let Some(index) = app_state.list_state.selected() {
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

        // ====================================================================
        // HANDLE EMOJI SELECTION (copy to clipboard + paste)
        // ====================================================================
        if let Some(emoji_value) = app_state.emoji_selected.take() {
            // Close the emoji picker UI state (already closed via close_emoji_picker,
            // but ensure it's clean)
            app_state.show_emoji_picker = false;

            // We need to exit the TUI, set clipboard, and paste
            // Store as a pseudo-selected entry so the exit logic handles it
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;

            if set_clipboard_text(&emoji_value, backend).is_ok() {
                println!("✓ Copied emoticon: {}", emoji_value);

                // Auto-paste
                if let Ok(exe) = std::env::current_exe() {
                    std::process::Command::new(exe).arg("--paste").spawn().ok();
                }
            }

            return Ok(());
        }

        // ====================================================================
        // HANDLE QUIT / SELECTION
        // ====================================================================
        if app_state.should_quit {
            // Capture selected entry before exiting if we were selecting
            if let Some(idx) = app_state.list_state.selected() {
                if let Some(entry) = filtered_entries.get(idx) {
                    // Only set if we actually "Selected" (pressed enter)
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
                std::process::Command::new(exe).arg("--paste").spawn().ok();
            }
        }
    }

    Ok(())
}
