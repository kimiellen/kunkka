use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 单次使用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub timestamp: u64,
    pub provider: String,
    pub model: String,
    pub role: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 使用统计汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummary {
    pub total_requests: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_tokens: u64,
    pub by_provider: HashMap<String, ProviderUsage>,
    pub by_role: HashMap<String, RoleUsage>,
}

/// 供应商使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsage {
    pub requests: u64,
    pub total_tokens: u64,
}

/// 角色使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleUsage {
    pub requests: u64,
    pub total_tokens: u64,
}

/// 使用量追踪器
pub struct UsageTracker {
    records: Arc<RwLock<Vec<UsageRecord>>>,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 记录一次使用
    pub async fn record(
        &self,
        provider: String,
        model: String,
        role: String,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) {
        let record = UsageRecord {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            provider,
            model,
            role,
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        };

        let mut records = self.records.write().await;
        records.push(record);
    }

    /// 获取使用统计汇总
    pub async fn summary(&self) -> UsageSummary {
        let records = self.records.read().await;

        let mut total_requests: u64 = 0;
        let mut total_prompt_tokens: u64 = 0;
        let mut total_completion_tokens: u64 = 0;
        let mut by_provider: HashMap<String, ProviderUsage> = HashMap::new();
        let mut by_role: HashMap<String, RoleUsage> = HashMap::new();

        for record in records.iter() {
            total_requests += 1;
            total_prompt_tokens += record.prompt_tokens as u64;
            total_completion_tokens += record.completion_tokens as u64;

            let provider_entry =
                by_provider
                    .entry(record.provider.clone())
                    .or_insert(ProviderUsage {
                        requests: 0,
                        total_tokens: 0,
                    });
            provider_entry.requests += 1;
            provider_entry.total_tokens += record.total_tokens as u64;

            let role_entry = by_role.entry(record.role.clone()).or_insert(RoleUsage {
                requests: 0,
                total_tokens: 0,
            });
            role_entry.requests += 1;
            role_entry.total_tokens += record.total_tokens as u64;
        }

        UsageSummary {
            total_requests,
            total_prompt_tokens,
            total_completion_tokens,
            total_tokens: total_prompt_tokens + total_completion_tokens,
            by_provider,
            by_role,
        }
    }

    /// 获取最近 N 条记录
    pub async fn recent(&self, limit: usize) -> Vec<UsageRecord> {
        let records = self.records.read().await;
        let start = if records.len() > limit {
            records.len() - limit
        } else {
            0
        };
        records[start..].to_vec()
    }

    /// 清空记录
    pub async fn clear(&self) {
        let mut records = self.records.write().await;
        records.clear();
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}
