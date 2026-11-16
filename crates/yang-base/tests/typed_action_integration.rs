//! Schema-first Action 端到端 CRUD 集成测试。
//!
//! 用 testcontainers 启动真实 MySQL，通过 `ModuleSpec::table(...).crud()` 注册
//! 全套内置 Action，再经 `BuiltApp::dispatch(...)` 跑完整 add → get → put → select →
//! del → table 流程。这条路径依赖 `ActionContext::table_query()` 从当前应用的
//! `Tools` 注入连接池——本测试同时验证该显式所有权链路。
//!
//! **需要 Docker**：CI（`CI=true` 或 `REQUIRE_DOCKER_TESTS=1`）下容器启动/端口映射/
//! 就绪等待失败会直接失败（fail-closed），不再零断言变绿；本地无 Docker 且未显式要求时
//! 按可辨识原因跳过。
//! 运行：`cargo test --test typed_action_integration -- --ignored --test-threads=1`
#![cfg(feature = "mysql")]
#![allow(deprecated)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::Arc;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_base::action::{Request, TokenAuthMiddleware, User};
use yang_base::definition::{
    ActionHandle, ActionName, ActionRef, AddonName, AddonSpec, AppBuilder, FieldName, Fields, Int,
    Key, ModuleName, ModuleSpec, Str, TableName, TableSpec,
};
use yang_base::table::Record;
use yang_base::token::TokenManager;
use yang_base::tools::{Tools, ToolsBuilder};
use yang_db::{redis::RedisConfig, Database, DatabaseConfig, RedisClient};

/// 构建端到端测试使用的运行期表定义。
fn test_table() -> TableSpec {
    let name = |value| FieldName::new(value).expect("测试字段名应有效");
    TableSpec::new(TableName::new("typed_test_users").expect("测试表名应有效")).fields(
        Fields::new()
            .field(name("id"), Key::new())
            .field(
                name("username"),
                Str::new().require(true).max_length(50).filterable(true),
            )
            .field(name("age"), Int::new().require(true).filterable(true)),
    )
}

/// 带登录 Token 的请求（内置非公开 Action 需要登录态）。
fn logged_in_request(body: serde_json::Value, tools: &Tools) -> Request {
    let token = tools
        .token()
        .expect("测试 Tools 应配置 TokenManager")
        .generate_access_token("tester", serde_json::json!({}))
        .expect("测试 access token 应生成成功");
    let req = Request::new(body).header("authorization", format!("Bearer {token}"));
    req
}

fn action_handle(app: &yang_base::definition::BuiltApp, name: &str) -> ActionHandle {
    let reference = ActionRef::new(
        ModuleName::new("test.user").expect("测试 Module 名称应有效"),
        ActionName::new(name).expect("测试 Action 名称应有效"),
    );
    app.registry().resolve(&reference).expect("Action 应已注册")
}

/// 显式要求 Docker 的环境开关（CI 脚本可设置；未设置时不影响本地开发）。
const REQUIRE_DOCKER_ENV: &str = "REQUIRE_DOCKER_TESTS";
/// 兼容 CI 编排使用的另一别名开关。
const YANG_REQUIRE_DOCKER_ENV: &str = "YANG_REQUIRE_DOCKER";
/// 唯一允许在 CI 中恢复「无 Docker 即跳过」的显式 opt-out。
const ALLOW_SKIP_ENV: &str = "ALLOW_SKIP_TESTCONTAINERS";

/// 把环境变量值解释为布尔开关；`0/false/no/off` 明确为 false。
fn env_flag_value(value: Option<String>) -> Option<bool> {
    value.map(|raw| {
        matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// 是否需要 fail-closed（环境不满足时直接失败）。
///
/// 注入 `lookup` 便于单元测试，避免多个测试并发读写进程级环境变量。
/// 优先级：显式 REQUIRE_* > 显式 ALLOW_SKIP_TESTCONTAINERS > CI 自动检测。
fn require_docker_tests_with(lookup: impl Fn(&str) -> Option<String>) -> bool {
    let flag = |name: &str| env_flag_value(lookup(name));
    if flag(REQUIRE_DOCKER_ENV) == Some(true) || flag(YANG_REQUIRE_DOCKER_ENV) == Some(true) {
        return true;
    }
    if flag(ALLOW_SKIP_ENV) == Some(true) {
        return false;
    }
    flag("CI") == Some(true)
}

/// 当前进程是否需要 fail-closed。
fn require_docker_tests() -> bool {
    require_docker_tests_with(|name| std::env::var(name).ok())
}

/// 环境不满足时：CI/显式要求模式直接 panic；本地模式打印可辨识原因后跳过。
///
/// RED 场景（修复前）：容器启动失败、端口读取失败或 30 秒超时都静默 return，
/// libtest 将测试记为通过，docker-mysql job 可以在零断言下变绿。
fn skip_or_fail<T>(require_docker: bool, reason: String) -> Option<T> {
    if require_docker {
        panic!("MySQL 集成测试要求可用的 Docker/MySQL（fail-closed）：{reason}");
    }
    eprintln!(
        "本地跳过 MySQL 集成测试（未检测到 CI 强制要求；可设置 {ALLOW_SKIP_ENV}=1 显式保留此行为）：{reason}"
    );
    None
}

/// 启动 MySQL/Redis 容器并构建显式应用资源；本地无 Docker 且未显式要求时返回 None。
async fn setup() -> Option<(
    testcontainers::ContainerAsync<GenericImage>,
    testcontainers::ContainerAsync<GenericImage>,
    Arc<Tools>,
)> {
    let require_docker = require_docker_tests();
    let image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db");

    let container = match image.start().await {
        Ok(c) => c,
        Err(error) => {
            return skip_or_fail(
                require_docker,
                format!("无法启动 MySQL Docker 容器: {error}"),
            );
        }
    };

    let mysql_id = container.id().to_string();
    let port = match container.get_host_port_ipv4(3306).await {
        Ok(port) => port,
        Err(error) => {
            return skip_or_fail(
                require_docker,
                format!("无法获取 MySQL 容器 {mysql_id} 的映射端口: {error}"),
            );
        }
    };
    let db_url = format!("mysql://root:test_password@127.0.0.1:{port}/test_db");

    // 等待 MySQL 就绪：500ms × 120 = 60 秒预算。
    let mut ready = false;
    let mut last_error = String::from("尚未尝试连接");
    for _ in 0..120 {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        match Database::connect(&db_url).await {
            Ok(db) => match db.execute("SELECT 1").await {
                Ok(_) => {
                    ready = true;
                    break;
                }
                Err(error) => last_error = format!("SELECT 1 失败: {error}"),
            },
            Err(error) => last_error = format!("连接失败: {error}"),
        }
    }
    if !ready {
        return skip_or_fail(
            require_docker,
            format!("MySQL 容器 {mysql_id}（端口 {port}）在 60 秒内未就绪，最后错误: {last_error}"),
        );
    }

    // 建表 + 构造当前应用持有的数据库（table_query 从 Tools 注入连接池）
    let db = match Database::connect_with_config(&db_url, DatabaseConfig::default()).await {
        Ok(db) => db,
        Err(error) => {
            return skip_or_fail(require_docker, format!("无法按显式配置连接 MySQL: {error}"));
        }
    };
    if let Err(error) = db
        .execute(
            "CREATE TABLE typed_test_users (\
                id BIGINT PRIMARY KEY AUTO_INCREMENT, \
                username VARCHAR(50) NOT NULL, \
                age INT NOT NULL)",
        )
        .await
    {
        return skip_or_fail(require_docker, format!("创建测试表失败: {error}"));
    }

    // TokenAuthMiddleware 使用黑名单校验，因此完整 dispatch 路径还需要 Redis。
    let redis_image = GenericImage::new("redis", "7-alpine").with_wait_for(
        testcontainers::core::WaitFor::message_on_stdout("Ready to accept connections"),
    );
    let redis_container = match redis_image.start().await {
        Ok(c) => c,
        Err(error) => {
            return skip_or_fail(
                require_docker,
                format!("无法启动 Redis Docker 容器: {error}"),
            );
        }
    };
    let redis_port = match redis_container.get_host_port_ipv4(6379).await {
        Ok(port) => port,
        Err(error) => {
            return skip_or_fail(
                require_docker,
                format!("无法获取 Redis 容器映射端口: {error}"),
            );
        }
    };
    let redis_url = format!("redis://127.0.0.1:{redis_port}");
    let cache = match RedisClient::connect_with_config(&redis_url, RedisConfig::default()).await {
        Ok(cache) => cache,
        Err(error) => {
            return skip_or_fail(require_docker, format!("Redis 连接失败: {error}"));
        }
    };

    let token = TokenManager::new_symmetric(
        "test_secret_keyxxxxxxxxxxxxxxxxx",
        jsonwebtoken::Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    )
    .expect("测试 TokenManager 应构建成功");
    let tools = match ToolsBuilder::new()
        .mysql(db)
        .cache(cache)
        .token(token)
        .build()
    {
        Ok(tools) => Arc::new(tools),
        Err(error) => {
            return skip_or_fail(require_docker, format!("构建测试 Tools 失败: {error}"));
        }
    };

    Some((container, redis_container, tools))
}

#[test]
fn docker_requirement_env_gate_keeps_ci_fail_closed_and_local_skip_explicit() {
    let lookup = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_string())
        }
    };

    // CI 默认 fail-closed：GitHub Actions 的 CI=true。
    assert!(require_docker_tests_with(lookup(&[("CI", "true")])));
    // 显式 REQUIRE_* 在非 CI 环境同样 fail-closed，兼容两种变量名。
    assert!(require_docker_tests_with(lookup(&[(
        REQUIRE_DOCKER_ENV,
        "1"
    )])));
    assert!(require_docker_tests_with(lookup(&[(
        YANG_REQUIRE_DOCKER_ENV,
        "yes"
    )])));
    // 本地非 CI 且无显式要求：保留带原因的跳过。
    assert!(!require_docker_tests_with(lookup(&[])));
    // CI=false 不等于 CI。
    assert!(!require_docker_tests_with(lookup(&[("CI", "false")])));
    // 唯一允许在 CI 中放行跳过的显式 opt-out。
    assert!(!require_docker_tests_with(lookup(&[
        ("CI", "true"),
        (ALLOW_SKIP_ENV, "1"),
    ])));
    // 显式 REQUIRE 优先于 opt-out，不能被绕过。
    assert!(require_docker_tests_with(lookup(&[
        (REQUIRE_DOCKER_ENV, "1"),
        (ALLOW_SKIP_ENV, "1"),
    ])));
}

#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn full_crud_cycle() {
    let (_mysql_container, _redis_container, tools) = match setup().await {
        Some(containers) => containers,
        None => return,
    };
    let module = ModuleSpec::new(ModuleName::new("test.user").expect("测试 Module 名称应有效"))
        .middleware(TokenAuthMiddleware::new(|claims| {
            User::new(1, claims.sub.clone()).with_permissions(["test.user:read", "test.user:write"])
        }))
        .table(test_table())
        .crud()
        .expect("schema-first CRUD 注册应成功");
    let app = AppBuilder::new()
        .addon(
            AddonSpec::new(AddonName::new("test").expect("测试 Addon 名称应有效")).module(module),
        )
        .build(Arc::clone(&tools))
        .expect("测试应用应构建成功");

    // 1. add
    let request = logged_in_request(serde_json::json!({"username": "alice", "age": 30}), &tools);
    let r = app
        .dispatch(action_handle(&app, "add"), request)
        .await
        .expect("add 应成功");
    assert_eq!(r.code, 0, "add code");
    assert_eq!(r.data.as_ref().unwrap()["affected"], 1);
    let inserted_id = r.data.as_ref().unwrap()["id"]
        .as_u64()
        .expect("add 应返回自增主键");

    // 2. get
    let request = logged_in_request(serde_json::json!({"id": inserted_id}), &tools);
    let r = app
        .dispatch(action_handle(&app, "get"), request)
        .await
        .expect("get 应成功");
    let user: Record = serde_json::from_value(r.data.unwrap()).expect("get 应返回 Record 对象");
    assert_eq!(
        user.require::<String>("username").expect("username 应存在"),
        "alice"
    );
    assert_eq!(user.require::<i32>("age").expect("age 应存在"), 30);

    // 3. put（data 为动态 Record 对象）
    let request = logged_in_request(
        serde_json::json!({"id": inserted_id, "data": {"age": 31}}),
        &tools,
    );
    let r = app
        .dispatch(action_handle(&app, "put"), request)
        .await
        .expect("put 应成功");
    assert_eq!(r.data.as_ref().unwrap()["affected"], 1);

    // 4. select（where like 叶子 + count_total）
    let request = logged_in_request(
        serde_json::json!({
            "page": 1, "page_size": 10,
            "where": {"type": "like", "field": "username", "pattern": "%alice%"},
            "count_total": true
        }),
        &tools,
    );
    let r = app
        .dispatch(action_handle(&app, "select"), request)
        .await
        .expect("select 应成功");
    let data = r.data.unwrap();
    let items: Vec<Record> =
        serde_json::from_value(data["items"].clone()).expect("items 应为 Record 数组");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].require::<i32>("age").expect("age 应存在"), 31);
    assert_eq!(data["total"], 1);

    // 4b. select（C2a OR 布尔组：username like %alice% OR age > 1000）
    let request = logged_in_request(
        serde_json::json!({
            "page": 1, "page_size": 10,
            "where": {"type": "or", "conditions": [
                {"type": "like", "field": "username", "pattern": "%alice%"},
                {"type": "gt", "field": "age", "value": 1000}
            ]},
            "count_total": true
        }),
        &tools,
    );
    let r = app
        .dispatch(action_handle(&app, "select"), request)
        .await
        .expect("select OR 应成功");
    let data = r.data.unwrap();
    assert_eq!(
        data["items"].as_array().unwrap().len(),
        1,
        "OR 组应匹配到 alice（age>1000 分支不命中）"
    );
    assert_eq!(data["total"], 1);

    // 5. del
    let request = logged_in_request(serde_json::json!({"id": inserted_id}), &tools);
    let r = app
        .dispatch(action_handle(&app, "del"), request)
        .await
        .expect("del 应成功");
    assert_eq!(r.data.as_ref().unwrap()["affected"], 1);

    // 6. table（返回运行期表定义元信息）
    let request = logged_in_request(serde_json::json!({}), &tools);
    let r = app
        .dispatch(action_handle(&app, "table"), request)
        .await
        .expect("table 应成功");
    let schema = r.data.unwrap();
    assert_eq!(schema["table_name"], "typed_test_users");
    assert_eq!(schema["primary_key"], "id");
}
