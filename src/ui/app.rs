use ratatui::widgets::ListState;

// ============================================================================
// TERMINAL UI APP STATE
// ============================================================================

pub struct AppState {
    pub list_state: ListState,
    pub should_quit: bool,
    pub selected_index: Option<usize>,
    pub selected_entry: Option<crate::models::ClipboardEntry>,
    pub show_clear_confirm: bool,
    pub is_searching: bool,
    pub search_query: String,
    /// Tracks which entry index is currently being revealed (for secrets)
    pub reveal_index: Option<usize>,
    /// Whether the emoji picker overlay is open
    pub show_emoji_picker: bool,
    /// Currently selected category index in the emoji picker
    pub emoji_category_index: usize,
    /// Currently selected item index within the active emoji category (flat index)
    pub emoji_item_index: usize,
    /// Number of columns in the emoji grid (updated each frame by the renderer)
    pub emoji_grid_cols: usize,
    /// Scroll offset (in rows) for the emoji grid viewport
    pub emoji_grid_scroll: usize,
    /// Search query for filtering emoticons in the picker
    pub emoji_search: String,
    /// The emoticon value selected by the user (to be copied to clipboard)
    pub emoji_selected: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        let mut state = Self {
            list_state: ListState::default(),
            should_quit: false,
            selected_index: None,
            selected_entry: None,
            show_clear_confirm: false,
            is_searching: false,
            search_query: String::new(),
            reveal_index: None,
            show_emoji_picker: false,
            emoji_category_index: 0,
            emoji_item_index: 0,
            emoji_grid_cols: 3,
            emoji_grid_scroll: 0,
            emoji_search: String::new(),
            emoji_selected: None,
        };
        state.list_state.select(Some(0));
        state
    }

    pub fn next(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| if i >= max - 1 { 0 } else { i + 1 })
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| if i == 0 { max - 1 } else { i - 1 })
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    pub fn select(&mut self) {
        self.selected_index = self.list_state.selected();
        self.should_quit = true;
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    // ========================================================================
    // EMOJI PICKER HELPERS
    // ========================================================================

    /// Open the emoji picker and reset its state.
    pub fn open_emoji_picker(&mut self) {
        self.show_emoji_picker = true;
        self.emoji_category_index = 0;
        self.emoji_item_index = 0;
        self.emoji_grid_scroll = 0;
        self.emoji_search.clear();
        self.emoji_selected = None;
    }

    /// Close the emoji picker UI overlay.
    /// Note: does NOT clear `emoji_selected` — that field is consumed
    /// separately by the copy/paste handler via `.take()`.
    pub fn close_emoji_picker(&mut self) {
        self.show_emoji_picker = false;
        self.emoji_search.clear();
    }

    // --------------------------------------------------------------------
    // Grid navigation
    //
    // The flat `emoji_item_index` is interpreted as row/col in a grid
    // with `emoji_grid_cols` columns.  Navigation wraps at boundaries.
    // After each move we call `ensure_grid_scroll_visible` so the
    // viewport follows the selection.
    // --------------------------------------------------------------------

    /// Move selection one cell to the right; wraps to next row, or to
    /// the first cell when the end is reached.
    pub fn emoji_grid_next_col(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        self.emoji_item_index = if self.emoji_item_index >= total - 1 {
            0
        } else {
            self.emoji_item_index + 1
        };
    }

    /// Move selection one cell to the left; wraps to previous row, or to
    /// the last cell when the start is reached.
    pub fn emoji_grid_prev_col(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        self.emoji_item_index = if self.emoji_item_index == 0 {
            total - 1
        } else {
            self.emoji_item_index - 1
        };
    }

    /// Move selection one full row down; if the target index exceeds the
    /// item count, clamp to the last item.  Wraps to the top when at the
    /// bottom.
    pub fn emoji_grid_next_row(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        let cols = self.emoji_grid_cols.max(1);
        let new_idx = self.emoji_item_index + cols;
        self.emoji_item_index = if new_idx >= total {
            // Wrap: keep the same column, land on the first row
            let col = self.emoji_item_index % cols;
            col.min(total - 1)
        } else {
            new_idx
        };
    }

    /// Move selection one full row up; wraps to the bottom when at the top.
    pub fn emoji_grid_prev_row(&mut self, total: usize) {
        if total == 0 {
            return;
        }
        let cols = self.emoji_grid_cols.max(1);
        if self.emoji_item_index >= cols {
            self.emoji_item_index -= cols;
        } else {
            // Wrap: keep the same column, land on the last row
            let col = self.emoji_item_index % cols;
            let total_rows = (total + cols - 1) / cols;
            let target = (total_rows - 1) * cols + col;
            self.emoji_item_index = if target >= total { total - 1 } else { target };
        }
    }

    /// Move to the next emoji category (wraps around).
    pub fn emoji_next_category(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        self.emoji_category_index = if self.emoji_category_index >= max - 1 {
            0
        } else {
            self.emoji_category_index + 1
        };
        self.emoji_item_index = 0;
        self.emoji_grid_scroll = 0;
    }

    /// Move to the previous emoji category (wraps around).
    pub fn emoji_prev_category(&mut self, max: usize) {
        if max == 0 {
            return;
        }
        self.emoji_category_index = if self.emoji_category_index == 0 {
            max - 1
        } else {
            self.emoji_category_index - 1
        };
        self.emoji_item_index = 0;
        self.emoji_grid_scroll = 0;
    }
}
