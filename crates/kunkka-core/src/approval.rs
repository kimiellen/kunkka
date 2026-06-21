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

    #[test]
    fn create_returns_sequential_ids() {
        let mut store = ApprovalStore::new();
        let id1 = store.create(
            "a".into(),
            "shell".into(),
            "cmd1".into(),
            vec!["cmd1".into()],
        );
        let id2 = store.create(
            "a".into(),
            "shell".into(),
            "cmd2".into(),
            vec!["cmd2".into()],
        );
        assert_eq!(id1, "approval-1");
        assert_eq!(id2, "approval-2");
        assert_ne!(id1, id2);
    }

    #[test]
    fn list_pending_returns_all_pending_entries() {
        let mut store = ApprovalStore::new();
        store.create(
            "notes".into(),
            "shell".into(),
            "cmd1".into(),
            vec!["cmd1".into()],
        );
        store.create(
            "notes".into(),
            "shell".into(),
            "cmd2".into(),
            vec!["cmd2".into()],
        );
        store.create(
            "todo".into(),
            "shell".into(),
            "cmd3".into(),
            vec!["cmd3".into()],
        );

        let pending = store.list_pending();
        assert_eq!(pending.len(), 3);
        assert!(pending.iter().all(|a| a.capability == "shell"));
    }

    #[test]
    fn list_pending_excludes_approved_entries() {
        let mut store = ApprovalStore::new();
        let id1 = store.create(
            "notes".into(),
            "shell".into(),
            "cmd1".into(),
            vec!["cmd1".into()],
        );
        store.create(
            "notes".into(),
            "shell".into(),
            "cmd2".into(),
            vec!["cmd2".into()],
        );

        store.approve(&id1);
        let pending = store.list_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].approval_id, "approval-2");
    }

    #[test]
    fn list_pending_excludes_rejected_entries() {
        let mut store = ApprovalStore::new();
        let id1 = store.create(
            "notes".into(),
            "shell".into(),
            "cmd1".into(),
            vec!["cmd1".into()],
        );
        store.create(
            "notes".into(),
            "shell".into(),
            "cmd2".into(),
            vec!["cmd2".into()],
        );

        store.reject(&id1);
        let pending = store.list_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].approval_id, "approval-2");
    }

    #[test]
    fn approve_nonexistent_id_is_noop() {
        let mut store = ApprovalStore::new();
        store.approve("nonexistent");
        assert!(store.list_pending().is_empty());
    }

    #[test]
    fn reject_nonexistent_id_is_noop() {
        let mut store = ApprovalStore::new();
        store.reject("nonexistent");
        assert!(store.list_pending().is_empty());
    }

    #[test]
    fn consume_approved_succeeds_for_approved_entry() {
        let mut store = ApprovalStore::new();
        let id = store.create(
            "notes".into(),
            "shell".into(),
            "rg todo".into(),
            vec!["rg".into()],
        );
        store.approve(&id);

        let result = store.consume_approved(&id, "notes", "shell", "rg todo");
        assert_eq!(result, Ok(()));
        assert!(!store.entries.contains_key(&id));
    }

    #[test]
    fn consume_approved_returns_pending_for_pending_entry() {
        let mut store = ApprovalStore::new();
        let id = store.create(
            "notes".into(),
            "shell".into(),
            "rg todo".into(),
            vec!["rg".into()],
        );

        let result = store.consume_approved(&id, "notes", "shell", "rg todo");
        assert_eq!(result, Err(ApprovalConsumeError::Pending));
    }

    #[test]
    fn consume_approved_returns_not_found_for_missing_entry() {
        let mut store = ApprovalStore::new();
        let result = store.consume_approved("missing", "notes", "shell", "cmd");
        assert_eq!(result, Err(ApprovalConsumeError::NotFound));
    }

    #[test]
    fn consume_approved_returns_mismatch_for_wrong_app_id() {
        let mut store = ApprovalStore::new();
        let id = store.create(
            "notes".into(),
            "shell".into(),
            "rg todo".into(),
            vec!["rg".into()],
        );
        store.approve(&id);

        let result = store.consume_approved(&id, "wrong-app", "shell", "rg todo");
        assert_eq!(result, Err(ApprovalConsumeError::Mismatch));
    }

    #[test]
    fn consume_approved_returns_mismatch_for_wrong_capability() {
        let mut store = ApprovalStore::new();
        let id = store.create(
            "notes".into(),
            "shell".into(),
            "rg todo".into(),
            vec!["rg".into()],
        );
        store.approve(&id);

        let result = store.consume_approved(&id, "notes", "fs", "rg todo");
        assert_eq!(result, Err(ApprovalConsumeError::Mismatch));
    }

    #[test]
    fn consume_approved_returns_mismatch_for_wrong_command() {
        let mut store = ApprovalStore::new();
        let id = store.create(
            "notes".into(),
            "shell".into(),
            "rg todo".into(),
            vec!["rg".into()],
        );
        store.approve(&id);

        let result = store.consume_approved(&id, "notes", "shell", "different cmd");
        assert_eq!(result, Err(ApprovalConsumeError::Mismatch));
    }

    #[test]
    fn consume_approved_returns_rejected_and_cleans_up() {
        let mut store = ApprovalStore::new();
        let id = store.create(
            "notes".into(),
            "shell".into(),
            "rm -rf /".into(),
            vec!["rm".into()],
        );
        store.reject(&id);

        let result = store.consume_approved(&id, "notes", "shell", "rm -rf /");
        assert_eq!(result, Err(ApprovalConsumeError::NotFound));
        assert!(!store.entries.contains_key(&id));
    }

    #[test]
    fn list_pending_preserves_field_values() {
        let mut store = ApprovalStore::new();
        store.create(
            "my-app".into(),
            "shell".into(),
            "printf 'hello world'".into(),
            vec!["printf".into()],
        );

        let pending = store.list_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].app_id, "my-app");
        assert_eq!(pending[0].capability, "shell");
        assert_eq!(pending[0].summary, "printf 'hello world'");
        assert!(pending[0].approval_id.starts_with("approval-"));
    }

    #[test]
    fn multiple_entries_independent_state_transitions() {
        let mut store = ApprovalStore::new();
        let id1 = store.create(
            "notes".into(),
            "shell".into(),
            "cmd1".into(),
            vec!["cmd1".into()],
        );
        let id2 = store.create(
            "notes".into(),
            "shell".into(),
            "cmd2".into(),
            vec!["cmd2".into()],
        );
        let id3 = store.create(
            "notes".into(),
            "shell".into(),
            "cmd3".into(),
            vec!["cmd3".into()],
        );

        store.approve(&id1);
        store.reject(&id2);

        let pending = store.list_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].approval_id, id3);

        assert_eq!(
            store.consume_approved(&id1, "notes", "shell", "cmd1"),
            Ok(())
        );
        assert_eq!(
            store.consume_approved(&id2, "notes", "shell", "cmd2"),
            Err(ApprovalConsumeError::NotFound)
        );
        assert_eq!(
            store.consume_approved(&id3, "notes", "shell", "cmd3"),
            Err(ApprovalConsumeError::Pending)
        );
    }
}
