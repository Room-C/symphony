use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use rand::Rng;

use crate::events::RetryEntry;

pub fn next_backoff(attempt: u32, max_ms: u64) -> Duration {
    let exponent = attempt.saturating_sub(1).min(16);
    let base = 10_000u64.saturating_mul(1u64 << exponent);
    let capped = base.min(max_ms);
    let jitter = if capped == 0 {
        0
    } else {
        rand::thread_rng().gen_range(0..=capped / 4)
    };
    Duration::from_millis(capped.saturating_sub(jitter))
}

pub fn continuation_delay() -> Duration {
    Duration::from_millis(1_000)
}

#[derive(Debug, Clone, Default)]
pub struct RetryQueue {
    entries: HashMap<String, RetryEntry>,
}

impl RetryQueue {
    pub fn schedule(
        &mut self,
        issue_id: impl Into<String>,
        identifier: impl Into<String>,
        attempt: u32,
        delay: Duration,
        error: Option<String>,
    ) -> RetryEntry {
        let issue_id = issue_id.into();
        let delay_ms = delay.as_millis().min(u128::from(u64::MAX)) as u64;
        let due_at = Utc::now() + TimeDelta::milliseconds(delay_ms.min(i64::MAX as u64) as i64);
        let entry = RetryEntry {
            issue_id: issue_id.clone(),
            identifier: identifier.into(),
            attempt,
            due_at,
            delay_ms,
            error,
        };
        self.entries.insert(issue_id, entry.clone());
        entry
    }

    pub fn pop_due(&mut self, now: DateTime<Utc>) -> Vec<RetryEntry> {
        let due_ids: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.due_at <= now)
            .map(|(id, _)| id.clone())
            .collect();
        due_ids
            .into_iter()
            .filter_map(|id| self.entries.remove(&id))
            .collect()
    }

    pub fn remove(&mut self, issue_id: &str) -> Option<RetryEntry> {
        self.entries.remove(issue_id)
    }

    pub fn entries(&self) -> &HashMap<String, RetryEntry> {
        &self.entries
    }
}
