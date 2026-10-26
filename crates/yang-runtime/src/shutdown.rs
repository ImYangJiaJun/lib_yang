//! 多阶段进程关闭的共享绝对截止时间。

use crate::observability::RuntimeMetricNames;
use std::error::Error as StdError;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{Mutex, Notify};
use tokio::task::{JoinError, JoinHandle};
use tokio::time::{timeout_at, Instant};

/// 保持后台任务所有权，等待被取消或启动中途失败时也不会把任务分离出去。
///
/// `join` 仅借用句柄；调用方取消关闭 future 时，所有者的 Drop 仍能中止任务。
/// Tokio 的 abort 是协作取消：不能中止已经运行的阻塞任务或不让出执行权的代码。
pub struct OwnedTask<T> {
    task: JoinHandle<T>,
}

impl<T> OwnedTask<T> {
    pub fn new(task: JoinHandle<T>) -> Self {
        Self { task }
    }

    /// 等待一次任务结束；future 被取消后可以再次等待，但完成后不得重复调用。
    pub async fn join(&mut self) -> Result<T, JoinError> {
        (&mut self.task).await
    }

    pub fn abort(&self) {
        self.task.abort();
    }
}

impl<T> Drop for OwnedTask<T> {
    fn drop(&mut self) {
        self.abort();
    }
}

#[derive(Debug, Error)]
pub enum ShutdownError {
    #[error("关闭阶段 {phase} 执行失败: {source}")]
    Phase {
        phase: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("关闭阶段 {phase} 达到共享截止时间（进程总预算 {total_ms} ms，含收尾预留）")]
    Timeout { phase: &'static str, total_ms: u128 },
}

#[derive(Clone)]
pub struct ShutdownBudget {
    total: Duration,
    shared: Arc<SharedState>,
    metric_names: RuntimeMetricNames,
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
            metric_names: RuntimeMetricNames::default(),
        }
    }

    /// 覆盖关闭阶段使用的指标名前缀（与 `LogIdentity::with_metric_names` 同形）。
    #[must_use]
    pub fn with_metric_names(mut self, metric_names: RuntimeMetricNames) -> Self {
        self.metric_names = metric_names;
        self
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
        metrics::counter!(self.metric_names.shutdown_started, "trigger" => trigger).increment(1);
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
        self.run_phase_with_reserve(phase, Duration::ZERO, future)
            .await
    }

    /// 从共享截止时间扣除收尾预留；并发阶段不会各自重新获得一份总预算。
    pub async fn run_phase_with_reserve<T, F>(
        &self,
        phase: &'static str,
        reserve: Duration,
        future: F,
    ) -> Result<T, ShutdownError>
    where
        F: Future<Output = anyhow::Result<T>>,
    {
        let mut window = loop {
            if let Some(window) = *self.shared.window.lock().await {
                break window;
            }
            self.wait_started().await;
        };
        window.deadline -= reserve.min(self.total);
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
                record_phase(
                    phase,
                    "success",
                    phase_started,
                    window.deadline,
                    self.metric_names,
                );
                Ok(value)
            }
            Ok(Err(source)) => {
                record_phase(
                    phase,
                    "error",
                    phase_started,
                    window.deadline,
                    self.metric_names,
                );
                Err(ShutdownError::Phase {
                    phase,
                    source: source.into_boxed_dyn_error(),
                })
            }
            Err(_) => {
                record_phase(
                    phase,
                    "timeout",
                    phase_started,
                    window.deadline,
                    self.metric_names,
                );
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
    metric_names: RuntimeMetricNames,
) {
    let now = Instant::now();
    let elapsed = now.saturating_duration_since(phase_started);
    metrics::counter!(metric_names.shutdown_phase, "phase" => phase, "result" => result)
        .increment(1);
    metrics::histogram!(metric_names.shutdown_phase_duration, "phase" => phase)
        .record(elapsed.as_secs_f64());
    tracing::info!(
        phase,
        result,
        elapsed_phase_ms = elapsed.as_millis() as u64,
        remaining_budget_ms = deadline.saturating_duration_since(now).as_millis() as u64,
        "关闭阶段结束"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    async fn pending_owned_task() -> anyhow::Result<(OwnedTask<()>, oneshot::Receiver<()>)> {
        let (started, ready) = oneshot::channel();
        let (resource, released) = oneshot::channel();
        let task = OwnedTask::new(tokio::spawn(async move {
            let _resource = resource;
            let _ = started.send(());
            std::future::pending::<()>().await;
        }));
        ready.await?;
        Ok((task, released))
    }

    #[tokio::test]
    async fn cancelled_join_keeps_ownership_until_drop() -> anyhow::Result<()> {
        let (mut task, released) = pending_owned_task().await?;
        assert!(tokio::time::timeout(Duration::from_millis(5), task.join())
            .await
            .is_err());
        drop(task);
        assert!(tokio::time::timeout(Duration::from_secs(1), released)
            .await
            .is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn startup_error_drops_spawned_task() -> anyhow::Result<()> {
        let (task, released) = pending_owned_task().await?;
        let startup = async move {
            let _task = task;
            Err::<(), _>(anyhow::anyhow!("模拟后续 worker 启动失败"))
        };
        assert!(startup.await.is_err());
        assert!(tokio::time::timeout(Duration::from_secs(1), released)
            .await
            .is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_phases_share_one_deadline_and_release_tasks() -> anyhow::Result<()> {
        let budget = ShutdownBudget::new(Duration::from_millis(20));
        let (mut first, first_released) = pending_owned_task().await?;
        let (mut second, second_released) = pending_owned_task().await?;
        let deadline = budget.begin("test").await;
        let (one, two) = tokio::join!(
            budget.run_phase("one", async move {
                first.join().await?;
                Ok(())
            }),
            budget.run_phase("two", async move {
                second.join().await?;
                Ok(())
            }),
        );
        assert!(matches!(one, Err(ShutdownError::Timeout { .. })));
        assert!(matches!(two, Err(ShutdownError::Timeout { .. })));
        assert_eq!(budget.begin("second_trigger").await, deadline);
        assert!(tokio::time::timeout(Duration::from_secs(1), first_released)
            .await
            .is_ok());
        assert!(
            tokio::time::timeout(Duration::from_secs(1), second_released)
                .await
                .is_ok()
        );
        Ok(())
    }

    #[tokio::test]
    async fn owned_task_propagates_completion_and_panic() -> anyhow::Result<()> {
        let mut success = OwnedTask::new(tokio::spawn(async { 42 }));
        assert_eq!(success.join().await?, 42);
        let mut failure = OwnedTask::new(tokio::spawn(async { panic!("worker panic") }));
        assert!(failure.join().await.is_err_and(|error| error.is_panic()));
        Ok(())
    }

    #[tokio::test]
    async fn drain_reserve_leaves_time_for_resource_cleanup() {
        let budget = ShutdownBudget::new(Duration::from_secs(1));
        budget.begin("test").await;
        let result = budget
            .run_phase_with_reserve(
                "drain",
                Duration::from_millis(900),
                std::future::pending::<anyhow::Result<()>>(),
            )
            .await;
        assert!(matches!(result, Err(ShutdownError::Timeout { .. })));
        assert!(budget
            .run_phase("cleanup", async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(())
            })
            .await
            .is_ok());
    }
}
