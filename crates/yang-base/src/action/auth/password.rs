//! 密码哈希与校验的受控执行边界。
//!
//! Argon2 是刻意的内存困难算法，直接运行在异步运行时上会阻塞工作线程；
//! [`PasswordEngine`] 把所有哈希/校验运算收敛到 `spawn_blocking`，并用信号量限制
//! 全局并发上限，避免认证洪泛耗尽阻塞线程池。并发上限等参数由构造函数注入，
//! 不读取任何应用配置。

use argon2::password_hash::{Error as PasswordHashError, PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use rand_core::OsRng;
use std::sync::Arc;
use tokio::sync::Semaphore;
use yang_base::BaseError;

/// 等时校验使用的固定 dummy PHC 哈希。
///
/// 算法与参数（argon2id / v=19 / m=19456,t=2,p=1）与 `Argon2::default()` 的产出
/// 完全一致，保证「账号不存在」与「密码错误」两条路径执行相同成本的 Argon2 运算；
/// 参数若漂移（例如升级 argon2 默认值），单元测试会拒绝启动并提示重新生成。
/// 该常量不对应任何真实账号，仅为拉齐认证入口的响应时间分布。
const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$KeT08Pv9+LVzDxDOmS18Tw$f1ITNKuxgUMXlkesQPqcEpIDZlQZ8FOm1wadvq9lxiU";

/// 密码哈希执行器：Argon2 运算 + 受控并发上限。
#[derive(Clone)]
pub struct PasswordEngine {
    permits: Arc<Semaphore>,
}

impl PasswordEngine {
    /// 创建执行器；`max_concurrency` 为同时进行的 Argon2 运算上限，必须大于 0。
    pub fn new(max_concurrency: usize) -> Result<Self, BaseError> {
        if max_concurrency == 0 {
            return Err(BaseError::ConfigError(
                "argon2_max_concurrency 必须大于 0".to_string(),
            ));
        }
        Ok(Self {
            permits: Arc::new(Semaphore::new(max_concurrency)),
        })
    }

    /// 计算密码的 Argon2 哈希（PHC 字符串格式）。
    pub async fn hash(&self, password: &str) -> Result<String, BaseError> {
        let password = password.to_owned();
        self.run_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(password.as_bytes(), &salt)
                .map(|value| value.to_string())
                .map_err(|_| BaseError::Unknown("密码哈希失败".to_string()))
        })
        .await
    }

    /// 校验密码明文与已存储的 PHC 哈希是否匹配。
    pub async fn verify(&self, password: &str, encoded: &str) -> Result<bool, BaseError> {
        let password = password.to_owned();
        let encoded = encoded.to_owned();
        self.run_blocking(move || {
            let parsed = PasswordHash::new(&encoded)
                .map_err(|_| BaseError::Unknown("数据库中的密码哈希格式无效".to_string()))?;
            match Argon2::default().verify_password(password.as_bytes(), &parsed) {
                Ok(()) => Ok(true),
                Err(PasswordHashError::Password) => Ok(false),
                Err(_) => Err(BaseError::Unknown("密码校验失败".to_string())),
            }
        })
        .await
    }

    /// 等时校验：账号记录缺失时也对内置 dummy 哈希执行一次完整的 Argon2 校验。
    ///
    /// 认证入口（如登录）在「用户不存在」与「密码错误」之间必须保持一致的响应时间
    /// 与响应错误，否则时间差会泄露用户名是否存在（时序枚举旁路）。传入
    /// `Some(真实哈希)` 时语义与 [`Self::verify`] 完全一致；传入 `None` 时走同一条
    /// 校验代码路径运算内置 dummy 哈希，并恒返回 `Ok(false)`——dummy 哈希
    /// 不对应任何真实账号，即使碰巧匹配也不能视为认证通过。
    pub async fn verify_or_dummy(
        &self,
        password: &str,
        encoded: Option<&str>,
    ) -> Result<bool, BaseError> {
        match encoded {
            Some(encoded) => self.verify(password, encoded).await,
            // 结果故意丢弃：None 分支只为拉齐耗时，语义上恒为校验失败。
            None => self
                .verify(password, DUMMY_PASSWORD_HASH)
                .await
                .map(|_| false),
        }
    }

    async fn run_blocking<T>(
        &self,
        operation: impl FnOnce() -> Result<T, BaseError> + Send + 'static,
    ) -> Result<T, BaseError>
    where
        T: Send + 'static,
    {
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .map_err(|_| BaseError::Unknown("密码执行器已关闭".to_string()))?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            operation()
        })
        .await
        .map_err(|error| BaseError::Unknown(format!("密码任务执行失败: {error}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn password_hash_round_trip() {
        let engine =
            PasswordEngine::new(1).unwrap_or_else(|error| panic!("密码执行器应构建成功: {error}"));
        let encoded = engine
            .hash("correct-horse-battery-staple")
            .await
            .unwrap_or_else(|error| panic!("密码应成功哈希: {error}"));

        assert!(engine
            .verify("correct-horse-battery-staple", &encoded)
            .await
            .unwrap_or_else(|error| panic!("密码应成功校验: {error}")));
        assert!(!engine
            .verify("wrong-password", &encoded)
            .await
            .unwrap_or_else(|error| panic!("错误密码应得到 false: {error}")));
        assert!(!encoded.contains("correct-horse-battery-staple"));
    }

    #[tokio::test]
    async fn dummy_hash_parameters_match_engine_output() {
        // 验证需求: 等时校验的前提是 dummy 哈希与真实哈希的 Argon2 参数一致，
        // 否则两条路径仍有可分辨的耗时差。这里用引擎真实产出做基准逐段比对。
        let engine =
            PasswordEngine::new(1).unwrap_or_else(|error| panic!("密码执行器应构建成功: {error}"));
        let real = engine
            .hash("parameter-probe")
            .await
            .unwrap_or_else(|error| panic!("密码应成功哈希: {error}"));
        // PHC 格式 $算法$v=版本$m=...,t=...,p=...$盐$哈希，前三段必须逐字相同。
        let dummy_segments: Vec<&str> = DUMMY_PASSWORD_HASH.split('$').take(4).collect();
        let real_segments: Vec<&str> = real.split('$').take(4).collect();
        assert_eq!(
            dummy_segments, real_segments,
            "dummy 哈希参数已漂移，请用 PasswordEngine::hash 重新生成 DUMMY_PASSWORD_HASH"
        );
    }

    #[tokio::test]
    async fn verify_or_dummy_runs_full_pipeline_for_none() {
        let engine =
            PasswordEngine::new(1).unwrap_or_else(|error| panic!("密码执行器应构建成功: {error}"));
        // None 分支必须走完整校验管线而非短路：若 dummy 哈希无法解析或未执行
        // Argon2 运算，这里会报错而不是返回 Ok(false)。
        assert!(!engine
            .verify_or_dummy("any-password", None)
            .await
            .unwrap_or_else(|error| panic!("None 分支应走完整校验并返回 false: {error}")));
        // dummy 哈希本身不认证任何密码。
        assert!(!engine
            .verify("any-password", DUMMY_PASSWORD_HASH)
            .await
            .unwrap_or_else(|error| panic!("dummy 哈希应能正常校验: {error}")));
    }

    #[tokio::test]
    async fn verify_or_dummy_matches_verify_for_real_hash() {
        let engine =
            PasswordEngine::new(1).unwrap_or_else(|error| panic!("密码执行器应构建成功: {error}"));
        let encoded = engine
            .hash("real-password")
            .await
            .unwrap_or_else(|error| panic!("密码应成功哈希: {error}"));
        assert!(engine
            .verify_or_dummy("real-password", Some(&encoded))
            .await
            .unwrap_or_else(|error| panic!("正确密码应通过等时校验: {error}")));
        assert!(!engine
            .verify_or_dummy("wrong-password", Some(&encoded))
            .await
            .unwrap_or_else(|error| panic!("错误密码应校验失败: {error}")));
    }

    #[test]
    fn zero_concurrency_is_rejected() {
        assert!(matches!(
            PasswordEngine::new(0),
            Err(BaseError::ConfigError(message)) if message.contains("argon2_max_concurrency")
        ));
    }
}
