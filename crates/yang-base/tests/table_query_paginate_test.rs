#![cfg(feature = "mysql")]
//! TableQuery 分页查询集成测试
//!
//! 测试 paginate 方法的完整功能，包括：
//! - 执行 COUNT 查询获取总记录数
//! - 计算 LIMIT 和 OFFSET
//! - 执行数据查询
//! - 构建 PaginatedResult 返回结果
//!
//! **验证需求**: 5.7
//!
//! **注意**: 这些测试需要 Docker 环境。CI（GitHub Actions 的 `CI=true`）或显式
//! `REQUIRE_DOCKER_TESTS=1` 下，容器启动/端口映射/就绪等待失败会直接失败（fail-closed），
//! 不再产生零断言绿灯；仅本地非 CI 环境按可辨识原因跳过，CI 中如需恢复旧的跳过行为
//! 必须显式设置 `ALLOW_SKIP_TESTCONTAINERS=1`。
//! 运行测试：`cargo test --test table_query_paginate_test -- --test-threads=1 --ignored`

#![allow(deprecated)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use sqlx::mysql::MySqlPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use yang_base::table::{
    Field, PaginatedResult, Record, SortOrder, Table, TableDefinition, TableQuery,
};
use yang_db::Database;

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
fn skip_or_fail(
    require_docker: bool,
    reason: String,
) -> Option<(testcontainers::ContainerAsync<GenericImage>, String)> {
    if require_docker {
        panic!("MySQL 集成测试要求可用的 Docker/MySQL（fail-closed）：{reason}");
    }
    eprintln!(
        "本地跳过 MySQL 集成测试（未检测到 CI 强制要求；可设置 {ALLOW_SKIP_ENV}=1 显式保留此行为）：{reason}"
    );
    None
}

/// 创建 MySQL 测试容器并返回数据库 URL
async fn setup_mysql() -> Option<(testcontainers::ContainerAsync<GenericImage>, String)> {
    let require_docker = require_docker_tests();
    let mysql_image = GenericImage::new("mysql", "8.0")
        .with_env_var("MYSQL_ROOT_PASSWORD", "test_password")
        .with_env_var("MYSQL_DATABASE", "test_db");

    let container = match mysql_image.start().await {
        Ok(c) => c,
        Err(error) => {
            return skip_or_fail(
                require_docker,
                format!("无法启动 MySQL Docker 容器: {error}"),
            );
        }
    };

    let container_id = container.id().to_string();
    let port = match container.get_host_port_ipv4(3306).await {
        Ok(port) => port,
        Err(error) => {
            return skip_or_fail(
                require_docker,
                format!("无法获取 MySQL 容器 {container_id} 的映射端口: {error}"),
            );
        }
    };
    let db_url = format!("mysql://root:test_password@127.0.0.1:{port}/test_db");

    if let Err(last_error) = wait_for_mysql(&db_url, 120).await {
        return skip_or_fail(
            require_docker,
            format!(
                "MySQL 容器 {container_id}（端口 {port}）在 60 秒内未就绪，最后错误: {last_error}"
            ),
        );
    }

    Some((container, db_url))
}

/// 等待 MySQL 完全启动；返回最后一次连接/SELECT 错误供失败诊断。
async fn wait_for_mysql(db_url: &str, max_retries: u32) -> Result<(), String> {
    let mut last_error = String::from("尚未尝试连接");
    for attempt in 1..=max_retries {
        // 500ms × 120 = 60 秒预算：比旧的 15×2s 更宽，同时更快发现已就绪实例。
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        match Database::connect(db_url).await {
            Ok(db) => match db.execute("SELECT 1").await {
                Ok(_) => {
                    println!("MySQL 已就绪（尝试 {attempt}/{max_retries}）");
                    return Ok(());
                }
                Err(error) => last_error = format!("SELECT 1 失败: {error}"),
            },
            Err(error) => last_error = format!("连接失败: {error}"),
        }

        if attempt % 10 == 0 || attempt == max_retries {
            println!("等待 MySQL 启动... ({attempt}/{max_retries})，最后错误: {last_error}");
        }
    }
    Err(last_error)
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

/// 创建测试表并插入测试数据
async fn setup_test_data(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    // 创建测试表
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_users (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(50) NOT NULL,
            email VARCHAR(100) NOT NULL,
            age INT NOT NULL
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
        "#,
    )
    .await?;

    // 插入测试数据（50 条记录）
    for i in 1..=50 {
        db.execute(&format!(
            "INSERT INTO test_users (name, email, age) VALUES ('User{}', 'user{}@example.com', {})",
            i,
            i,
            20 + (i % 30)
        ))
        .await?;
    }

    Ok(())
}

/// 创建测试用的表定义。
fn create_test_table_definition() -> TableDefinition {
    Table::new("test_users")
        .fields([
            Field::id("id"),
            Field::string("name", 50).required(),
            Field::string("email", 100).required(),
            Field::integer("age").required(),
        ])
        .build()
        .expect("test_users 表定义应有效")
}

fn bound_query(
    definition: TableDefinition,
    roles: Arc<[String]>,
    pool: Option<Arc<sqlx::MySqlPool>>,
) -> TableQuery {
    definition
        .bind(pool.expect("测试查询必须绑定连接池"))
        .query(roles.iter().cloned())
}

/// 测试基本分页查询
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_basic() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 创建数据库连接池
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    // 创建测试数据
    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    // 创建表配置和查询
    let table_config = create_test_table_definition();

    let query = bound_query(
        table_config,
        vec!["user".to_string()].into(),
        Some(Arc::new(pool)),
    );

    // 执行分页查询：第 1 页，每页 10 条
    let result: PaginatedResult<Record> =
        query.page(1, 10).unwrap().paginate_records().await.unwrap();

    // 验证结果
    assert_eq!(result.total, 50, "总记录数应该是 50");
    assert_eq!(result.page, 1, "当前页应该是 1");
    assert_eq!(result.page_size, 10, "每页大小应该是 10");
    assert_eq!(result.total_pages, 5, "总页数应该是 5");
    assert_eq!(result.data.len(), 10, "当前页数据条数应该是 10");
    assert!(result.has_next(), "应该有下一页");
    assert!(!result.has_prev(), "不应该有上一页");
}

/// 测试分页查询第 2 页
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_second_page() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_definition();

    let query = bound_query(
        table_config,
        vec!["user".to_string()].into(),
        Some(Arc::new(pool)),
    );

    // 执行分页查询：第 2 页，每页 10 条
    let result: PaginatedResult<Record> =
        query.page(2, 10).unwrap().paginate_records().await.unwrap();

    // 验证结果
    assert_eq!(result.total, 50);
    assert_eq!(result.page, 2);
    assert_eq!(result.page_size, 10);
    assert_eq!(result.total_pages, 5);
    assert_eq!(result.data.len(), 10);
    assert!(result.has_next(), "应该有下一页");
    assert!(result.has_prev(), "应该有上一页");
}

/// 测试分页查询最后一页
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_last_page() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_definition();

    let query = bound_query(
        table_config,
        vec!["user".to_string()].into(),
        Some(Arc::new(pool)),
    );

    // 执行分页查询：第 5 页（最后一页），每页 10 条
    let result: PaginatedResult<Record> =
        query.page(5, 10).unwrap().paginate_records().await.unwrap();

    // 验证结果
    assert_eq!(result.total, 50);
    assert_eq!(result.page, 5);
    assert_eq!(result.page_size, 10);
    assert_eq!(result.total_pages, 5);
    assert_eq!(result.data.len(), 10);
    assert!(!result.has_next(), "不应该有下一页");
    assert!(result.has_prev(), "应该有上一页");
}

/// 测试分页查询空结果
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_empty_result() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    // 创建数据库连接池
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();

    // 创建空表
    db.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_users (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(50) NOT NULL,
            email VARCHAR(100) NOT NULL,
            age INT NOT NULL
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
        "#,
    )
    .await
    .unwrap();

    let table_config = create_test_table_definition();

    let query = bound_query(
        table_config,
        vec!["user".to_string()].into(),
        Some(Arc::new(pool)),
    );

    // 执行分页查询
    let result: PaginatedResult<Record> =
        query.page(1, 10).unwrap().paginate_records().await.unwrap();

    // 验证结果
    assert_eq!(result.total, 0, "总记录数应该是 0");
    assert_eq!(result.page, 1);
    assert_eq!(result.page_size, 10);
    assert_eq!(result.total_pages, 0, "总页数应该是 0");
    assert_eq!(result.data.len(), 0, "数据列表应该为空");
    assert!(!result.has_next());
    assert!(!result.has_prev());
}

/// 测试分页查询带 WHERE 条件
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_with_where_condition() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_definition();

    let query = bound_query(
        table_config,
        vec!["user".to_string()].into(),
        Some(Arc::new(pool)),
    );

    // 执行分页查询：age = 25
    let result: PaginatedResult<Record> = query
        .where_eq("age", serde_json::json!(25))
        .unwrap()
        .page(1, 10)
        .unwrap()
        .paginate_records()
        .await
        .unwrap();

    // 验证结果：应该有一些记录（age = 25 的记录）
    assert!(result.total > 0, "应该有符合条件的记录");
    assert_eq!(result.page, 1);
    assert_eq!(result.page_size, 10);
    assert!(result.data.len() <= 10);

    // 验证所有返回的记录都满足条件
    for user in &result.data {
        assert_eq!(
            user.require::<i64>("age").unwrap(),
            25,
            "所有记录的 age 应该等于 25"
        );
    }
}

/// 测试分页查询带排序
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_with_order_by() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_definition();

    let query = bound_query(
        table_config,
        vec!["user".to_string()].into(),
        Some(Arc::new(pool)),
    );

    // 执行分页查询：按 id 降序排列
    let result: PaginatedResult<Record> = query
        .order_by("id", SortOrder::Desc)
        .unwrap()
        .page(1, 10)
        .unwrap()
        .paginate_records()
        .await
        .unwrap();

    // 验证结果
    assert_eq!(result.total, 50);
    assert_eq!(result.data.len(), 10);

    // 验证排序：第一条记录的 id 应该是最大的
    assert_eq!(
        result.data[0].require::<i64>("id").unwrap(),
        50,
        "第一条记录的 id 应该是 50"
    );
    assert_eq!(
        result.data[9].require::<i64>("id").unwrap(),
        41,
        "第十条记录的 id 应该是 41"
    );

    // 验证降序排列
    for i in 0..result.data.len() - 1 {
        assert!(
            result.data[i].require::<i64>("id").unwrap()
                > result.data[i + 1].require::<i64>("id").unwrap(),
            "记录应该按 id 降序排列"
        );
    }
}

/// 测试分页查询带字段选择
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_with_field_selection() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_definition();

    let query = bound_query(
        table_config,
        vec!["user".to_string()].into(),
        Some(Arc::new(pool)),
    );

    // 执行分页查询：选择所有字段
    let result: PaginatedResult<Record> = query
        .select_fields(&["id", "name", "email", "age"])
        .unwrap()
        .page(1, 10)
        .unwrap()
        .paginate_records()
        .await
        .unwrap();

    // 验证结果
    assert_eq!(result.total, 50);
    assert_eq!(result.data.len(), 10);

    // 验证字段值存在
    for user in &result.data {
        assert!(user.require::<i64>("id").unwrap() > 0, "id 应该大于 0");
        assert!(
            !user.require::<String>("name").unwrap().is_empty(),
            "name 不应该为空"
        );
    }
}

/// 测试分页查询使用默认分页参数
///
/// **验证需求**: 5.7
#[tokio::test]
#[ignore] // 需要 Docker 环境
async fn test_paginate_with_default_params() {
    let (_container, db_url) = match setup_mysql().await {
        Some(setup) => setup,
        None => return,
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db_url)
        .await
        .unwrap();

    let db = Database::connect(&db_url).await.unwrap();
    setup_test_data(&db).await.unwrap();

    let table_config = create_test_table_definition();

    let query = bound_query(
        table_config,
        vec!["user".to_string()].into(),
        Some(Arc::new(pool)),
    );

    // 执行分页查询：不设置分页参数，使用默认值
    let result: PaginatedResult<Record> = query.paginate_records().await.unwrap();

    // 验证结果：默认应该是第 1 页，每页 10 条
    assert_eq!(result.total, 50);
    assert_eq!(result.page, 1, "默认页码应该是 1");
    assert_eq!(result.page_size, 10, "默认每页大小应该是 10");
    assert_eq!(result.total_pages, 5, "总页数应该是 5");
    assert_eq!(result.data.len(), 10, "当前页数据条数应该是 10");
}
