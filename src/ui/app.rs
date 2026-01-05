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
}
