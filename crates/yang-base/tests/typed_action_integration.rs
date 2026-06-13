//! 类型化 Action 端到端 CRUD 集成测试（H-1 验收）
//!
//! 用 testcontainers 启动真实 MySQL，通过 `ModuleRouter::table_typed::<T>()` 注册
//! 全套内置 Action，再经 `router.dispatch(...)` 跑完整 add → get → put → select →
//! del → table 流程。这条路径依赖 `ActionContext::table_query()` 从 `GlobalDatabase`
//! 注入连接池——本测试同时验证该注入链路。
//!
//! **需要 Docker**：无 Docker 时静默跳过（与既有集成测试一致）。
//! 运行：`cargo test --test typed_action_integration -- --ignored --test-threads=1`
#![cfg(feature = "mysql")]

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_base::action::{ActionContext, GlobalTools, Request, User};
use yang_base::database::GlobalDatabase;
use yang_base::router::ModuleRouter;
use yang_base::token::TokenManager;
use yang_base_derive::TableEntity;
use yang_db::{Database, DatabaseConfig};

/// 端到端测试实体。
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, sqlx::FromRow, TableEntity)]
#[table(name = "typed_test_users")]
struct U {
    #[entity(primary_key)]
    id: i64,
    #[entity(max_length = 50)]
    username: String,
    age: i32,
}

/// 创建测试用 GlobalTools（对称密钥 TokenManager）。
fn test_tools() -> Arc<GlobalTools> {
    let tm = TokenManager::new_symmetric(
        "test_secret_key",
        jsonwebtoken::Algorithm::HS256,
        "test_issuer".to_string(),
        "test_audience".to_string(),
        3600,
        86400,
    );
    Arc::new(GlobalTools::new(tm))
}

/// 已登录用户上下文（内置非公开 Action 需要登录态）。
fn logged_in_ctx(body: serde_json::Value, tools: Arc<GlobalTools>) -> ActionContext {
    let req = Request::new(body);
    ActionContext::new(req, tools).with_user(User::new(1, "tester"))
}

/// 启动 MySQL 容器并初始化 GlobalDatabase；无 Docker 时返回 None。
async fn setup() -> Option<testcontainers::ContainerAsync<GenericImage>> {
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

    // 建表 + 初始化全局数据库（table_query 从此处注入连接池）
    let db = Database::connect(&db_url).await.ok()?;
    db.execute(
        "CREATE TABLE typed_test_users (\
            id BIGINT PRIMARY KEY AUTO_INCREMENT, \
            username VARCHAR(50) NOT NULL, \
            age INT NOT NULL)",
    )
    .await
    .ok()?;

    GlobalDatabase::init(&db_url, DatabaseConfig::default())
        .await
        .ok()?;

    Some(container)
}

#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn full_crud_cycle() {
    let _container = match setup().await {
        Some(c) => c,
        None => return,
    };
    let tools = test_tools();
    let router = ModuleRouter::new("user", "用户")
        .with_table_config(Arc::new(
            <U as yang_base::table::TableEntity>::table_config().clone(),
        ))
        .table_typed::<U>()
        .expect("table_typed 注册应成功");

    // 1. add
    let ctx = logged_in_ctx(
        serde_json::json!({"id": 1, "username": "alice", "age": 30}),
        tools.clone(),
    );
    let r = router.dispatch("add", ctx).await.expect("add 应成功");
    assert_eq!(r.code, 0, "add code");
    assert_eq!(r.data.as_ref().unwrap()["affected"], 1);

    // 2. get
    let ctx = logged_in_ctx(serde_json::json!({"id": 1}), tools.clone());
    let r = router.dispatch("get", ctx).await.expect("get 应成功");
    let user: U = serde_json::from_value(r.data.unwrap()).unwrap();
    assert_eq!(user.username, "alice");
    assert_eq!(user.age, 30);

    // 3. put（data 为 [字段, 值] 对列表）
    let ctx = logged_in_ctx(
        serde_json::json!({"id": 1, "data": [["age", 31]]}),
        tools.clone(),
    );
    let r = router.dispatch("put", ctx).await.expect("put 应成功");
    assert_eq!(r.data.as_ref().unwrap()["affected"], 1);

    // 4. select（where like 叶子 + count_total）
    let ctx = logged_in_ctx(
        serde_json::json!({
            "page": 1, "page_size": 10,
            "where": {"field": "username", "cond": {"op": "like", "value": "%alice%"}},
            "count_total": true
        }),
        tools.clone(),
    );
    let r = router.dispatch("select", ctx).await.expect("select 应成功");
    let data = r.data.unwrap();
    assert_eq!(data["items"].as_array().unwrap().len(), 1);
    assert_eq!(data["items"][0]["age"], 31);
    assert_eq!(data["total"], 1);

    // 4b. select（C2a OR 布尔组：username like %alice% OR age > 1000）
    let ctx = logged_in_ctx(
        serde_json::json!({
            "page": 1, "page_size": 10,
            "where": {"or": [
                {"field": "username", "cond": {"op": "like", "value": "%alice%"}},
                {"field": "age", "cond": {"op": "gt", "value": 1000}}
            ]},
            "count_total": true
        }),
        tools.clone(),
    );
    let r = router.dispatch("select", ctx).await.expect("select OR 应成功");
    let data = r.data.unwrap();
    assert_eq!(
        data["items"].as_array().unwrap().len(),
        1,
        "OR 组应匹配到 alice（age>1000 分支不命中）"
    );
    assert_eq!(data["total"], 1);

    // 5. del
    let ctx = logged_in_ctx(serde_json::json!({"id": 1}), tools.clone());
    let r = router.dispatch("del", ctx).await.expect("del 应成功");
    assert_eq!(r.data.as_ref().unwrap()["affected"], 1);

    // 6. table（公开 Action，返回元信息）
    let ctx = logged_in_ctx(serde_json::json!({}), tools.clone());
    let r = router.dispatch("table", ctx).await.expect("table 应成功");
    let schema = r.data.unwrap();
    assert_eq!(schema["table_name"], "typed_test_users");
    assert_eq!(schema["primary_key"], "id");
}
