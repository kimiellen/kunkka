#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    Ping,
    Approvals,
}

pub enum PingStatus {
    Idle,
    Loading,
    Ok(String),
    Err(String),
}

#[derive(Debug)]
pub enum ApprovalsStatus {
    Idle,
    Loading,
    Loaded,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct PendingApprovalItem {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub summary: String,
}

pub struct App {
    pub should_quit: bool,
    pub ping_status: PingStatus,
    pub current_view: View,
    pub approvals: Vec<PendingApprovalItem>,
    pub selected_index: usize,
    pub approvals_status: ApprovalsStatus,
}

impl Default for App {
    fn default() -> Self {
        Self {
            should_quit: false,
            ping_status: PingStatus::Idle,
            current_view: View::Approvals,
            approvals: Vec::new(),
            selected_index: 0,
            approvals_status: ApprovalsStatus::Idle,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle between Ping and Approvals views.
    pub fn toggle_view(&mut self) {
        self.current_view = match self.current_view {
            View::Ping => View::Approvals,
            View::Approvals => View::Ping,
        };
    }

    /// Move selection up in the approvals list. No-op if empty or at top.
    pub fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down in the approvals list. No-op if empty or at bottom.
    pub fn move_selection_down(&mut self) {
        if !self.approvals.is_empty() && self.selected_index < self.approvals.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Replace the approvals list and clamp the selected index.
    pub fn set_approvals(&mut self, items: Vec<PendingApprovalItem>) {
        self.approvals = items;
        self.approvals_status = ApprovalsStatus::Loaded;
        if self.selected_index >= self.approvals.len() {
            self.selected_index = self.approvals.len().saturating_sub(1);
        }
    }

    /// Record an approval decision result. On success the list is refreshed;
    /// on failure the error is stored in `approvals_status`.
    pub fn apply_approval_result(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                // Mark status as loading so a refresh is triggered.
                self.approvals_status = ApprovalsStatus::Loading;
            }
            Err(msg) => {
                self.approvals_status = ApprovalsStatus::Error(msg);
            }
        }
    }

    /// Return a reference to the currently selected approval, if any.
    pub fn selected_approval(&self) -> Option<&PendingApprovalItem> {
        self.approvals.get(self.selected_index)
    }
}
