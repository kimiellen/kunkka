use kunkka_protocol::core_control::PendingApproval;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const APPROVAL_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRecord {
    pub approval_id: String,
    pub app_id: String,
    pub capability: String,
    pub command: String,
    pub commands: Vec<String>,
    pub state: ApprovalState,
    pub created_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalConsumeError {
    NotFound,
    Pending,
    Rejected,
    Expired,
    Mismatch,
}

#[derive(Debug, Default)]
pub struct ApprovalStore {
    entries: BTreeMap<String, ApprovalRecord>,
    next_id: u64,
}

impl ApprovalStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(
        &mut self,
        app_id: String,
        capability: String,
        command: String,
        commands: Vec<String>,
    ) -> String {
        self.next_id += 1;
        let approval_id = format!("approval-{}", self.next_id);
        self.entries.insert(
            approval_id.clone(),
            ApprovalRecord {
                approval_id: approval_id.clone(),
                app_id,
                capability,
                command,
                commands,
                state: ApprovalState::Pending,
                created_at: Instant::now(),
            },
        );
        approval_id
    }

    pub fn list_pending(&mut self) -> Vec<PendingApproval> {
        self.expire_pending_entries(Instant::now());
        let approvals = self
            .entries
            .values()
            .filter(|entry| entry.state == ApprovalState::Pending)
            .map(|entry| PendingApproval {
                approval_id: entry.approval_id.clone(),
                app_id: entry.app_id.clone(),
                capability: entry.capability.clone(),
                summary: entry.command.clone(),
            })
            .collect();
        self.drop_terminal_entries();
        approvals
    }

    pub fn approve(&mut self, approval_id: &str) {
        self.expire_pending_entries(Instant::now());
        if let Some(entry) = self.entries.get_mut(approval_id) {
            entry.state = ApprovalState::Approved;
        }
        self.drop_terminal_entries();
    }

    pub fn reject(&mut self, approval_id: &str) {
        self.expire_pending_entries(Instant::now());
        if let Some(entry) = self.entries.get_mut(approval_id) {
            entry.state = ApprovalState::Rejected;
        }
        self.drop_terminal_entries();
    }

    pub fn expire(&mut self, approval_id: &str) {
        if let Some(entry) = self.entries.get_mut(approval_id) {
            entry.state = ApprovalState::Expired;
        }
    }

    pub fn consume_approved(
        &mut self,
        approval_id: &str,
        app_id: &str,
        capability: &str,
        command: &str,
    ) -> Result<(), ApprovalConsumeError> {
        self.expire_pending_entries(Instant::now());
        let Some(entry) = self.entries.get(approval_id) else {
            return Err(ApprovalConsumeError::NotFound);
        };

        if entry.app_id != app_id || entry.capability != capability || entry.command != command {
            return Err(ApprovalConsumeError::Mismatch);
        }

        match entry.state {
            ApprovalState::Pending => Err(ApprovalConsumeError::Pending),
            ApprovalState::Rejected => {
                self.entries.remove(approval_id);
                Err(ApprovalConsumeError::Rejected)
            }
            ApprovalState::Expired => {
                self.entries.remove(approval_id);
                Err(ApprovalConsumeError::Expired)
            }
            ApprovalState::Approved => {
                self.entries.remove(approval_id);
                Ok(())
            }
        }
    }

    fn expire_pending_entries(&mut self, now: Instant) {
        for entry in self.entries.values_mut() {
            if entry.state == ApprovalState::Pending
                && now.duration_since(entry.created_at) >= APPROVAL_TTL
            {
                entry.state = ApprovalState::Expired;
            }
        }
    }

    fn drop_terminal_entries(&mut self) {
        self.entries.retain(|_, entry| {
            !matches!(
                entry.state,
                ApprovalState::Rejected | ApprovalState::Expired
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_pending_skips_expired_entries_after_lazy_reap() {
        let mut store = ApprovalStore::new();
        let approval_id = store.create(
            "notes".to_string(),
            "shell".to_string(),
            "printf later".to_string(),
            vec!["printf".to_string()],
        );

        let entry = store.entries.get_mut(&approval_id).unwrap();
        entry.created_at = Instant::now() - APPROVAL_TTL;

        let approvals = store.list_pending();
        assert!(approvals.is_empty());
        assert!(!store.entries.contains_key(&approval_id));
    }

    #[test]
    fn consume_approved_returns_expired_for_stale_pending_entry() {
        let mut store = ApprovalStore::new();
        let approval_id = store.create(
            "notes".to_string(),
            "shell".to_string(),
            "printf later".to_string(),
            vec!["printf".to_string()],
        );

        let entry = store.entries.get_mut(&approval_id).unwrap();
        entry.created_at = Instant::now() - APPROVAL_TTL;

        let result = store.consume_approved(&approval_id, "notes", "shell", "printf later");
        assert_eq!(result, Err(ApprovalConsumeError::Expired));
        assert!(!store.entries.contains_key(&approval_id));
    }

    #[test]
    fn approved_entry_is_not_expired_by_pending_ttl() {
        let mut store = ApprovalStore::new();
        let approval_id = store.create(
            "notes".to_string(),
            "shell".to_string(),
            "printf approved".to_string(),
            vec!["printf".to_string()],
        );

        let entry = store.entries.get_mut(&approval_id).unwrap();
        entry.state = ApprovalState::Approved;
        entry.created_at = Instant::now() - APPROVAL_TTL;

        let result = store.consume_approved(&approval_id, "notes", "shell", "printf approved");
        assert_eq!(result, Ok(()));
    }
}
