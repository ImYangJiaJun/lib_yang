#![allow(clippy::expect_used)]

//! 邮箱验证码引擎 `verify_only`（只验不消费）的 Redis 集成测试。
//!
//! 对抗性目标（两段式登录方案 A 的实现红线）：
//! - peek 成功不销毁验证码，且随后 `consume` 仍是原子单次消费；
//! - peek 失败路径与 `consume` 共享错误计数与上限销毁语义，
//!   不存在不计次的暴力枚举旁路；
//! - 所有失败统一为 `ParamInvalid(email_code)`（防枚举形状）。

use std::sync::{Arc, Mutex};

use testcontainers::{runners::AsyncRunner, GenericImage};
use yang_base::action::auth::{
    EmailDeliveryError, EmailVerificationConfig, RegistrationEmailSender,
    RegistrationEmailSenderHandle, RegistrationEmailVerification,
};
use yang_base::action::{ActionContext, Request};
use yang_base::error::BaseError;
use yang_base::tools::ToolsBuilder;
use yang_db::RedisClient;

const KEY_PREFIX: &str = "yang-base:test:email-verify";
const MAX_ATTEMPTS: u32 = 3;

#[derive(Clone, Default)]
struct CapturingSender {
    codes: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl RegistrationEmailSender for CapturingSender {
    async fn send_registration_code(
        &self,
        recipient: &str,
        code: &str,
        expires_in_seconds: u64,
    ) -> Result<(), EmailDeliveryError> {
        let _ = (recipient, expires_in_seconds);
        self.codes
            .lock()
            .expect("捕获锁不应中毒")
            .push(code.to_string());
        Ok(())
    }
}

struct Fixture {
    _container: testcontainers::ContainerAsync<GenericImage>,
    cache: RedisClient,
    ctx: ActionContext,
    sender: CapturingSender,
}

async fn fixture() -> Fixture {
    let image = GenericImage::new("redis", "7-alpine").with_wait_for(
        testcontainers::core::WaitFor::message_on_stdout("Ready to accept connections"),
    );
    let container = image.start().await.expect("应能启动 Redis 7 测试容器");
    let port = container
        .get_host_port_ipv4(6379)
        .await
        .expect("应能读取 Redis 映射端口");
    let cache = RedisClient::connect(format!("redis://127.0.0.1:{port}"))
        .await
        .expect("应能连接 Redis 测试容器");
    let sender = CapturingSender::default();
    let tools = ToolsBuilder::new()
        .cache(cache.clone())
        .extension(RegistrationEmailSenderHandle::new(sender.clone()))
        .build()
        .expect("Redis 与投递器应冻结为 Tools");
    let ctx = ActionContext::new(Request::new(serde_json::json!({})), Arc::new(tools));
    Fixture {
        _container: container,
        cache,
        ctx,
        sender,
    }
}

fn test_config() -> EmailVerificationConfig {
    EmailVerificationConfig {
        redis_key_prefix: KEY_PREFIX.to_string(),
        secret: "email-verification-integration-secret-32b".to_string(),
        ttl_seconds: 600,
        resend_cooldown_seconds: 60,
        max_attempts: MAX_ATTEMPTS,
        code_digits: 6,
        send_window_seconds: 3600,
        send_ip_attempts: 100,
        send_email_attempts: 100,
        send_global_attempts: 1000,
        send_metric_name: "test_email_verification_send_total",
        verify_metric_name: "test_email_verification_verify_total",
    }
}

fn engine(config: &EmailVerificationConfig) -> RegistrationEmailVerification<'_> {
    RegistrationEmailVerification::from_config(config).expect("测试配置应合法")
}

/// 签发一枚验证码并返回捕获的明文码。
async fn issue_code(fixture: &Fixture, config: &EmailVerificationConfig, email: &str) -> String {
    engine(config)
        .request(&fixture.ctx, email, true)
        .await
        .expect("应能接受验证码签发请求");
    fixture
        .sender
        .codes
        .lock()
        .expect("捕获锁不应中毒")
        .last()
        .expect("投递器应捕获到验证码")
        .clone()
}

/// 生成一枚与 `code` 不同的合法形态错误验证码。
fn wrong_code(code: &str) -> String {
    let flipped = if code.starts_with('0') { '1' } else { '0' };
    format!("{flipped}{}", &code[1..])
}

async fn assert_invalid_code(result: Result<(), BaseError>) {
    let error = result.expect_err("失败必须统一为无效验证码错误");
    assert!(
        matches!(&error, BaseError::ParamInvalid(field, _) if field == "email_code"),
        "失败形状必须是 ParamInvalid(email_code)，实际为: {error:?}"
    );
}

async fn live_code_keys(cache: &RedisClient) -> Vec<String> {
    cache
        .keys(&format!("{KEY_PREFIX}:code:*"))
        .await
        .expect("应能枚举验证码 key")
}

#[tokio::test]
#[ignore = "需要 Docker 启动 Redis 7"]
async fn verify_only_peeks_without_consuming_and_consume_stays_atomic() {
    let fixture = fixture().await;
    let config = test_config();
    let email = "peek@example.com";
    let code = issue_code(&fixture, &config, email).await;

    // peek 成功不销毁：连续两次 peek 均通过，key 仍存在。
    engine(&config)
        .verify_only(&fixture.ctx, email, &code)
        .await
        .expect("第一次 peek 应通过");
    engine(&config)
        .verify_only(&fixture.ctx, email, &code)
        .await
        .expect("peek 不得消费验证码，第二次仍应通过");
    assert_eq!(live_code_keys(&fixture.cache).await.len(), 1);

    // peek 之后 consume 仍是原子单次消费。
    engine(&config)
        .consume(&fixture.ctx, email, &code)
        .await
        .expect("peek 后 consume 应通过");
    assert_eq!(live_code_keys(&fixture.cache).await.len(), 0);
    assert_invalid_code(engine(&config).consume(&fixture.ctx, email, &code).await).await;
    assert_invalid_code(
        engine(&config)
            .verify_only(&fixture.ctx, email, &code)
            .await,
    )
    .await;
}

#[tokio::test]
#[ignore = "需要 Docker 启动 Redis 7"]
async fn verify_only_shares_attempt_counting_and_limit_destruction() {
    let fixture = fixture().await;
    let config = test_config();
    let email = "attempts@example.com";
    let code = issue_code(&fixture, &config, email).await;
    let wrong = wrong_code(&code);

    // 形态非法的验证码不进 Redis、不计数（与 consume 的预检一致）。
    assert_invalid_code(engine(&config).verify_only(&fixture.ctx, email, "12").await).await;

    // 错误尝试照常计数：上限 3 次内正确验证码仍可 peek。
    for _ in 0..MAX_ATTEMPTS - 1 {
        assert_invalid_code(
            engine(&config)
                .verify_only(&fixture.ctx, email, &wrong)
                .await,
        )
        .await;
    }
    engine(&config)
        .verify_only(&fixture.ctx, email, &code)
        .await
        .expect("未达上限时正确验证码仍应通过");

    // 再错一次达上限即销毁：之后正确验证码对 peek/consume 都无效，
    // 证明 peek 不是不计次的暴力枚举旁路。
    assert_invalid_code(
        engine(&config)
            .verify_only(&fixture.ctx, email, &wrong)
            .await,
    )
    .await;
    assert_eq!(live_code_keys(&fixture.cache).await.len(), 0);
    assert_invalid_code(
        engine(&config)
            .verify_only(&fixture.ctx, email, &code)
            .await,
    )
    .await;
    assert_invalid_code(engine(&config).consume(&fixture.ctx, email, &code).await).await;
}
