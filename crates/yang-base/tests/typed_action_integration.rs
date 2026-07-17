//! Schema-first Action 端到端 CRUD 集成测试。
//!
//! 用 testcontainers 启动真实 MySQL，通过 `ModuleSpec::table(...).crud()` 注册
//! 全套内置 Action，再经 `BuiltApp::dispatch(...)` 跑完整 add → get → put → select →
//! del → table 流程。这条路径依赖 `ActionContext::table_query()` 从当前应用的
//! `Tools` 注入连接池——本测试同时验证该显式所有权链路。
//!
//! **需要 Docker**：无 Docker 时静默跳过（与既有集成测试一致）。
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
            .field(name("username"), Str::new().require(true).max_length(50))
            .field(name("age"), Int::new().require(true)),
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

/// 启动 MySQL/Redis 容器并构建显式应用资源；无 Docker 时返回 None。
async fn setup() -> Option<(
    testcontainers::ContainerAsync<GenericImage>,
    testcontainers::ContainerAsync<GenericImage>,
    Arc<Tools>,
)> {
    let image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db");

    let container = match image.start().await {
        Ok(c) => c,
        Err(e) => {
            println!("跳过测试：无法启动 Docker 容器: {}", e);
            return None;
        }
    };

    let port = container.get_host_port_ipv4(3306).await.ok()?;
    let db_url = format!("mysql://root:test_password@127.0.0.1:{}/test_db", port);

    // 等待 MySQL 就绪
    let mut ready = false;
    for _ in 0..15 {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        if let Ok(db) = Database::connect(&db_url).await {
            if db.execute("SELECT 1").await.is_ok() {
                ready = true;
                break;
            }
        }
    }
    if !ready {
        println!("跳过测试：MySQL 容器启动超时");
        return None;
    }

    // 建表 + 构造当前应用持有的数据库（table_query 从 Tools 注入连接池）
    let db = Database::connect_with_config(&db_url, DatabaseConfig::default())
        .await
        .ok()?;
    db.execute(
        "CREATE TABLE typed_test_users (\
            id BIGINT PRIMARY KEY AUTO_INCREMENT, \
            username VARCHAR(50) NOT NULL, \
            age INT NOT NULL)",
    )
    .await
    .ok()?;

    // TokenAuthMiddleware 使用黑名单校验，因此完整 dispatch 路径还需要 Redis。
    let redis_image = GenericImage::new("redis", "7-alpine").with_wait_for(
        testcontainers::core::WaitFor::message_on_stdout("Ready to accept connections"),
    );
    let redis_container = match redis_image.start().await {
        Ok(c) => c,
        Err(e) => {
            println!("跳过测试：无法启动 Redis 容器: {}", e);
            return None;
        }
    };
    let redis_port = redis_container.get_host_port_ipv4(6379).await.ok()?;
    let redis_url = format!("redis://127.0.0.1:{}", redis_port);
    let cache = RedisClient::connect_with_config(&redis_url, RedisConfig::default())
        .await
        .ok()?;

    let token = TokenManager::new_symmetric(
        "test_secret_key",
        jsonwebtoken::Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    );
    let tools = Arc::new(
        ToolsBuilder::new()
            .database(db)
            .cache(cache)
            .token(token)
            .build()
            .ok()?,
    );

    Some((container, redis_container, tools))
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
