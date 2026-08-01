//! 多阶段进程关闭的共享绝对截止时间。

use std::error::Error as StdError;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{Mutex, Notify};
use tokio::time::{timeout_at, Instant};

#[derive(Debug, Error)]
pub enum ShutdownError {
    #[error("关闭阶段 {phase} 执行失败: {source}")]
    Phase {
        phase: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("关闭阶段 {phase} 耗尽进程总预算（{total_ms} ms）")]
    Timeout { phase: &'static str, total_ms: u128 },
}

#[derive(Clone)]
pub struct ShutdownBudget {
    total: Duration,
    shared: Arc<SharedState>,
}

struct SharedState {
    window: Mutex<Option<ShutdownWindow>>,
    started: Notify,
}

#[derive(Debug, Clone, Copy)]
struct ShutdownWindow {
    started_at: Instant,
    deadline: Instant,
}

impl ShutdownBudget {
    pub fn new(total: Duration) -> Self {
        Self {
            total,
            shared: Arc::new(SharedState {
                window: Mutex::new(None),
                started: Notify::new(),
            }),
        }
    }

    pub async fn begin(&self, trigger: &'static str) -> Instant {
        let mut guard = self.shared.window.lock().await;
        if let Some(window) = *guard {
            return window.deadline;
        }
        let started_at = Instant::now();
        let deadline = started_at + self.total;
        *guard = Some(ShutdownWindow {
            started_at,
            deadline,
        });
        drop(guard);
        self.shared.started.notify_waiters();
        metrics::counter!("yang_runtime_shutdown_started_total", "trigger" => trigger).increment(1);
        tracing::info!(
            trigger,
            total_budget_ms = self.total.as_millis() as u64,
            "进程关闭总预算已启动"
        );
        deadline
    }

    pub async fn wait_started(&self) -> Instant {
        loop {
            let notified = self.shared.started.notified();
            if let Some(window) = *self.shared.window.lock().await {
                return window.deadline;
            }
            notified.await;
        }
    }

    pub async fn run_phase<T, F>(&self, phase: &'static str, future: F) -> Result<T, ShutdownError>
    where
        F: Future<Output = anyhow::Result<T>>,
    {
        let window = loop {
            if let Some(window) = *self.shared.window.lock().await {
                break window;
            }
            self.wait_started().await;
        };
        let phase_started = Instant::now();
        tracing::info!(
            phase,
            elapsed_total_ms = phase_started
                .saturating_duration_since(window.started_at)
                .as_millis() as u64,
            remaining_budget_ms = window
                .deadline
                .saturating_duration_since(phase_started)
                .as_millis() as u64,
            "开始执行关闭阶段"
        );
        match timeout_at(window.deadline, future).await {
            Ok(Ok(value)) => {
                record_phase(phase, "success", phase_started, window.deadline);
                Ok(value)
            }
            Ok(Err(source)) => {
                record_phase(phase, "error", phase_started, window.deadline);
                Err(ShutdownError::Phase {
                    phase,
                    source: source.into_boxed_dyn_error(),
                })
            }
            Err(_) => {
                record_phase(phase, "timeout", phase_started, window.deadline);
                Err(ShutdownError::Timeout {
                    phase,
                    total_ms: self.total.as_millis(),
                })
            }
        }
    }
}

fn record_phase(
    phase: &'static str,
    result: &'static str,
    phase_started: Instant,
    deadline: Instant,
) {
    let now = Instant::now();
    let elapsed = now.saturating_duration_since(phase_started);
    metrics::counter!("yang_runtime_shutdown_phase_total", "phase" => phase, "result" => result)
        .increment(1);
    metrics::histogram!("yang_runtime_shutdown_phase_duration_seconds", "phase" => phase)
        .record(elapsed.as_secs_f64());
    tracing::info!(
        phase,
        result,
        elapsed_phase_ms = elapsed.as_millis() as u64,
        remaining_budget_ms = deadline.saturating_duration_since(now).as_millis() as u64,
        "关闭阶段结束"
    );
}
