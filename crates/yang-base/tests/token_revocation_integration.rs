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
            .verify_token_checked(token)
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
    );
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
        .verify_token_checked(&token)
        .await
        .expect("合法且早于当前 Token 的水位线不应误杀");
    assert_eq!(claims.sub, subject);

    cache
        .del(&[watermark_key])
        .await
        .expect("应能清理测试水位线");
    tools.close().await;
}
