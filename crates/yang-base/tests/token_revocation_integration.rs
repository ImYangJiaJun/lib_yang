#![allow(clippy::expect_used)]

use jsonwebtoken::Algorithm;
use serde_json::json;
use testcontainers::{runners::AsyncRunner, GenericImage};
use yang_base::error::BaseError;
use yang_base::token::TokenManager;
use yang_base::tools::{Tools, ToolsBuilder};
use yang_db::RedisClient;

async fn redis_container() -> (testcontainers::ContainerAsync<GenericImage>, RedisClient) {
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
    (container, cache)
}

async fn assert_public_paths_fail_closed(
    tools: &Tools,
    subject: &str,
    token: &str,
    leaked_fragment: Option<&str>,
) {
    for error in [
        tools
            .token()
            .expect("Tools 应包含 TokenManager")
            .subject_min_iat(subject)
            .await
            .expect_err("公开水位线读取必须拒绝损坏值"),
        tools
            .token()
            .expect("Tools 应包含 TokenManager")
            .verify_token_checked(token, yang_base::token::TokenType::Access)
            .await
            .expect_err("鉴权路径必须拒绝损坏水位线"),
    ] {
        assert!(
            matches!(&error, BaseError::TokenRevocationStateInvalid(_)),
            "损坏撤销状态必须返回结构化错误，实际为: {error:?}"
        );
        assert_eq!(error.code(), 400008);
        assert_eq!(error.code_str(), "400008");
        assert!(error.is_server_error());
        if let Some(fragment) = leaked_fragment {
            assert!(
                !error.to_string().contains(fragment),
                "结构化错误不得回显 Redis 损坏原值"
            );
        }
    }
}

#[tokio::test]
#[ignore = "需要 Docker 启动 Redis 7"]
async fn corrupt_subject_watermark_fails_closed_on_public_verification_paths() {
    let (_container, cache) = redis_container().await;
    let subject = "b02-corrupt-watermark";
    let manager = TokenManager::new_symmetric(
        "b02_token_revocation_integration_secret",
        Algorithm::HS256,
        "b02-issuer".to_string(),
        "b02-audience".to_string(),
        3_600,
        86_400,
    )
    .expect("测试 TokenManager 应构建成功");
    let tools = ToolsBuilder::new()
        .cache(cache.clone())
        .token(manager)
        .build()
        .expect("TokenManager 与 Redis 应冻结为同一 Tools");
    let token = tools
        .token()
        .expect("Tools 应包含 TokenManager")
        .generate_access_token(subject, json!({"role": "user"}))
        .expect("应能生成固定 Access Token");

    tools
        .token()
        .expect("Tools 应包含 TokenManager")
        .revoke_by_subject(subject)
        .await
        .expect("应能通过公开 API 写入用户撤销水位线");
    let mut watermark_keys = cache
        .keys("token:user:*:min_iat")
        .await
        .expect("应能定位隔离容器中的水位线 key");
    assert_eq!(watermark_keys.len(), 1, "隔离容器中应只有一个水位线 key");
    let watermark_key = watermark_keys.pop().expect("水位线 key 已确认存在");

    cache
        .set(&watermark_key, "secret-corrupt-watermark")
        .await
        .expect("应能注入损坏水位线");

    assert_public_paths_fail_closed(&tools, subject, &token, Some("secret-corrupt-watermark"))
        .await;

    let write_binary = cache.script("redis.call('SET', KEYS[1], string.char(255)); return 1");
    let write_result: i64 = cache
        .eval_script(&write_binary, std::slice::from_ref(&watermark_key), &[])
        .await
        .expect("应能注入非 UTF-8 二进制水位线");
    assert_eq!(write_result, 1);
    assert_public_paths_fail_closed(&tools, subject, &token, None).await;

    cache
        .set(&watermark_key, "0")
        .await
        .expect("应能恢复合法水位线");
    let claims = tools
        .token()
        .expect("Tools 应包含 TokenManager")
        .verify_token_checked(&token, yang_base::token::TokenType::Access)
        .await
        .expect("合法且早于当前 Token 的水位线不应误杀");
    assert_eq!(claims.sub, subject);

    cache
        .del(&[watermark_key])
        .await
        .expect("应能清理测试水位线");
    tools.close().await;
}

/// 复用检测（refresh token 家族撤销）：旧 Refresh Token 被重放时，
/// 必须拒绝本次轮换并撤销该用户全部会话（水位线生效，重放前签发的
/// access token 立即失效）。
#[tokio::test]
#[ignore = "需要 Docker 启动 Redis 7"]
async fn refresh_token_replay_triggers_family_revocation() {
    let (_container, cache) = redis_container().await;
    let subject = "b02-replay-detection";
    let manager = TokenManager::new_symmetric(
        "b02_replay_detection_integration_secret",
        Algorithm::HS256,
        "b02-issuer".to_string(),
        "b02-audience".to_string(),
        3_600,
        86_400,
    )
    .expect("测试 TokenManager 应构建成功");
    let tools = ToolsBuilder::new()
        .cache(cache.clone())
        .token(manager)
        .build()
        .expect("TokenManager 与 Redis 应冻结为同一 Tools");
    let token_manager = || tools.token().expect("Tools 应包含 TokenManager");

    let (access_token, refresh_token) = token_manager()
        .generate_token_pair(subject, json!({}))
        .expect("应能生成 Token 对");

    // 首次轮换：合法使用，必须成功
    let (rotated_access, _rotated_refresh) = token_manager()
        .rotate_refresh_token(&refresh_token, json!({}))
        .await
        .expect("首次轮换应成功");

    // 重放同一个旧 Refresh Token：必须拒绝并触发家族撤销
    let replay = token_manager()
        .rotate_refresh_token(&refresh_token, json!({}))
        .await;
    assert!(
        matches!(replay, Err(BaseError::TokenRevoked)),
        "重放旧 Refresh Token 必须返回 TokenRevoked，实际: {replay:?}"
    );

    // 家族撤销生效：重放前签发的旧 access token 立即失效
    let old_access = token_manager()
        .verify_token_checked(&access_token, yang_base::token::TokenType::Access)
        .await;
    assert!(
        matches!(old_access, Err(BaseError::TokenRevoked)),
        "家族撤销后旧 access token 必须失效，实际: {old_access:?}"
    );
    // 含等号水位线（iat <= min_iat）的同秒连坐是刻意的安全取向（见
    // rotate_refresh_token 文档「复用检测语义」）：同秒签发的新 Token 对一并失效。
    let rotated_access_check = token_manager()
        .verify_token_checked(&rotated_access, yang_base::token::TokenType::Access)
        .await;
    assert!(
        matches!(rotated_access_check, Err(BaseError::TokenRevoked)),
        "同秒签发的新 Token 对应被水位线一并撤销，实际: {rotated_access_check:?}"
    );

    let watermark_keys = cache
        .keys("token:user:*:min_iat")
        .await
        .expect("应能定位隔离容器中的水位线 key");
    cache
        .del(&watermark_keys)
        .await
        .expect("应能清理测试水位线");
    tools.close().await;
}
