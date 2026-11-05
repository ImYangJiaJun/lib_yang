//! OAuth `state` 存储。
//!
//! state 一次性、10 分钟即废，失败闭合：进程重启丢掉未消费的 state，管理员重新点
//! 「去授权」即可。crate 不提供进程级全局单例；宿主自行持有单个实例。

use std::collections::HashMap;
use std::sync::Mutex;

/// state 有效期（秒）。
pub const STATE_TTL_SECONDS: i64 = 600;

/// state 的签发与消费。
pub trait StateStore: Send + Sync {
    /// 签发一个新 state 并记下有效期。
    fn mint(&self, now: i64) -> String;
    /// 校验并**消费**一个 state：命中即删，过期或未签发一律不认。
    fn consume(&self, state: &str, now: i64) -> bool;
}

/// 进程内 state 存储。
#[derive(Debug, Default)]
pub struct InMemoryStateStore {
    states: Mutex<HashMap<String, i64>>,
}

impl StateStore for InMemoryStateStore {
    fn mint(&self, now: i64) -> String {
        let state = uuid::Uuid::new_v4().simple().to_string();
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        states.retain(|_, expires_at| *expires_at > now);
        states.insert(state.clone(), now + STATE_TTL_SECONDS);
        state
    }

    fn consume(&self, state: &str, now: i64) -> bool {
        let mut states = self
            .states
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match states.remove(state) {
            Some(expires_at) => expires_at > now,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_is_single_use_and_expires() {
        let store = InMemoryStateStore::default();
        let state = store.mint(1_000);
        assert!(store.consume(&state, 1_001));
        assert!(!store.consume(&state, 1_001));

        let expired = store.mint(2_000);
        assert!(!store.consume(&expired, 2_000 + STATE_TTL_SECONDS + 1));
        assert!(!store.consume("never-issued", 2_000));
    }
}
