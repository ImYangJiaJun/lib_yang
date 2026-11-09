#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL Phase B 集成测试：数组绑定 / JSONB 谓词 / INET 系列类型 / BYTEA 往返。
//!
//! 所有测试均标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test integration_pg_phase_b_types -- --ignored --test-threads=1
//! ```

use serde_json::json;
use yang_db::postgres::field::SESSION_GUC_BASELINE;
use yang_db::postgres::{Condition, Database, FieldType, QueryBuilder, SqlValue};
use yang_db::{field, table, CompareOp, SortOrder};

fn test_db_url() -> String {
    std::env::var("PG_TEST_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:55432/test".to_string())
}

async fn connect(url: &str) -> Database {
    Database::connect(url)
        .await
        .expect("PG_TEST_URL 指向的 PostgreSQL 必须可连接")
}

async fn prepare_schema(db: &Database) {
    db.execute_with_params("CREATE SCHEMA IF NOT EXISTS phase_b_it", vec![])
        .await
        .expect("创建测试 schema 应成功");
}

async fn drop_schema(db: &Database) {
    db.execute_with_params("DROP SCHEMA phase_b_it CASCADE", vec![])
        .await
        .expect("清理测试 schema 应成功");
}

#[derive(Debug, sqlx::FromRow)]
struct LabelRow {
    label: String,
}

/// 按 id 升序取回匹配行的 label，便于断言条件命中的行集合。
async fn select_labels(builder: QueryBuilder<'_>) -> Vec<String> {
    builder
        .field(field!("label"))
        .order(field!("id"), SortOrder::Asc)
        .select::<LabelRow>()
        .await
        .expect("条件筛选查询应成功")
        .into_iter()
        .map(|row| row.label)
        .collect()
}

async fn prepare_arrays_table(db: &Database) {
    prepare_schema(db).await;
    db.execute_with_params("DROP TABLE IF EXISTS phase_b_it.arrays", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_it.arrays (
            id BIGSERIAL PRIMARY KEY,
            label TEXT NOT NULL,
            score DOUBLE PRECISION NOT NULL,
            active BOOLEAN NOT NULL,
            tags TEXT[] NOT NULL,
            nums BIGINT[] NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建数组测试表应成功");
    db.execute_with_params(
        "INSERT INTO phase_b_it.arrays (label, score, active, tags, nums) VALUES
            ('r1', 1.5, TRUE,  ARRAY['a','b'], ARRAY[1,2]),
            ('r2', 3.5, FALSE, ARRAY['c'],     ARRAY[3]),
            ('r3', 5.0, FALSE, ARRAY[]::text[], ARRAY[]::bigint[])",
        vec![],
    )
    .await
    .expect("插入数组测试数据应成功");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_arrays_any_all_and_binding() {
    let db = connect(&test_db_url()).await;
    prepare_arrays_table(&db).await;

    let arrays = || db.table(table!("phase_b_it.arrays"));

    // ANY：`id = ANY($1)`，Vec<i64> → BIGINT[] 单参数绑定。
    assert_eq!(
        select_labels(arrays().where_any(field!("id"), vec![1_i64, 3])).await,
        vec!["r1".to_string(), "r3".to_string()]
    );

    // ANY 的补集由 NOT 直接表达，确认渲染侧组合正确。
    assert_eq!(
        select_labels(arrays().where_not(Condition::Any(
            "id".to_string(),
            SqlValue::from(vec![1_i64, 3]),
        )))
        .await,
        vec!["r2".to_string()]
    );

    // 字符串数组：`label = ANY($1)`，Vec<String> → TEXT[] 单参数绑定。
    assert_eq!(
        select_labels(arrays().where_any(field!("label"), vec!["r2".to_string()])).await,
        vec!["r2".to_string()]
    );

    // 浮点数组。
    assert_eq!(
        select_labels(arrays().where_any(field!("score"), vec![1.5_f64, 3.5])).await,
        vec!["r1".to_string(), "r2".to_string()]
    );

    // 布尔数组。
    assert_eq!(
        select_labels(arrays().where_any(field!("active"), vec![false])).await,
        vec!["r2".to_string(), "r3".to_string()]
    );

    // ALL：单元素与多元素；`id = ALL(ARRAY[1,2])` 不存在同时等于两者的行。
    assert_eq!(
        select_labels(arrays().where_all(field!("id"), vec![1_i64])).await,
        vec!["r1".to_string()]
    );
    assert!(
        select_labels(arrays().where_all(field!("id"), vec![1_i64, 2]))
            .await
            .is_empty()
    );

    // 数组相等：`tags = $1`，数组作为单个绑定参数。
    assert_eq!(
        select_labels(
            arrays()
                .where_and(
                    field!("tags"),
                    CompareOp::Eq,
                    SqlValue::from(vec!["a".to_string(), "b".to_string()]),
                )
                .expect("受控数组相等条件应合法")
        )
        .await,
        vec!["r1".to_string()]
    );
    assert_eq!(
        select_labels(
            arrays()
                .where_and(field!("nums"), CompareOp::Eq, SqlValue::from(vec![3_i64]),)
                .expect("受控数组相等条件应合法")
        )
        .await,
        vec!["r2".to_string()]
    );

    drop_schema(&db).await;
}

async fn prepare_docs_table(db: &Database) {
    prepare_schema(db).await;
    db.execute_with_params("DROP TABLE IF EXISTS phase_b_it.docs", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_it.docs (
            id BIGSERIAL PRIMARY KEY,
            label TEXT NOT NULL,
            data JSONB NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建 JSONB 测试表应成功");
    db.execute_with_params(
        "INSERT INTO phase_b_it.docs (label, data) VALUES
            ('r1', '{\"role\":\"admin\",\"active\":true,\"weird''key\":1}'::jsonb),
            ('r2', '{\"role\":\"user\",\"active\":true}'::jsonb),
            ('r3', '{\"role\":\"admin\",\"active\":false,\"nested\":{\"x\":1}}'::jsonb)",
        vec![],
    )
    .await
    .expect("插入 JSONB 测试数据应成功");
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_jsonb_predicates() {
    let db = connect(&test_db_url()).await;
    prepare_docs_table(&db).await;

    let docs = || db.table(table!("phase_b_it.docs"));

    // `data @> $1`：JSON 包含。
    assert_eq!(
        select_labels(docs().where_json_contains(field!("data"), json!({"role": "admin"}))).await,
        vec!["r1".to_string(), "r3".to_string()]
    );
    assert_eq!(
        select_labels(docs().where_json_contains(field!("data"), json!({"active": true}))).await,
        vec!["r1".to_string(), "r2".to_string()]
    );

    // `data <@ $1`：JSON 被包含。容器需覆盖 r1 的全部键值。
    assert_eq!(
        select_labels(docs().where_json_contained_by(
            field!("data"),
            json!({
                "role": "admin",
                "active": true,
                "weird'key": 1,
                "nested": {"x": 1},
            }),
        ))
        .await,
        vec!["r1".to_string()]
    );

    // `data ? $1`：键存在；键名走绑定参数（含引号的键验证不拼接 SQL 文本）。
    assert_eq!(
        select_labels(docs().where_json_has_key(field!("data"), "weird'key")).await,
        vec!["r1".to_string()]
    );
    assert_eq!(
        select_labels(docs().where_json_has_key(field!("data"), "nested")).await,
        vec!["r3".to_string()]
    );
    // 注入形态的键名必须作为字面键查询而命中 0 行，而不是改变谓词语义。
    assert!(
        select_labels(docs().where_json_has_key(field!("data"), "role' OR '1'='1"))
            .await
            .is_empty()
    );

    // JSON NULL 值渲染为 SQL NULL：`NULL` 谓词不命中任何行。
    assert!(
        select_labels(docs().where_json_contains(field!("data"), SqlValue::Null))
            .await
            .is_empty()
    );

    // NOT 组合：不是 admin 的行。
    assert_eq!(
        select_labels(docs().where_not(Condition::JsonContains(
            "data".to_string(),
            SqlValue::Json(json!({"role": "admin"})),
        )))
        .await,
        vec!["r2".to_string()]
    );

    drop_schema(&db).await;
}

fn type_name(field_type: FieldType) -> String {
    field_type
        .postgres_type_name()
        .expect("测试用字段类型必须有 PostgreSQL 类型名")
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_network_types_and_array_type_names() {
    let db = connect(&test_db_url()).await;
    prepare_schema(&db).await;
    db.execute_with_params("DROP TABLE IF EXISTS phase_b_it.networks", vec![])
        .await
        .expect("删除旧表应成功");

    // 直接使用 FieldType 映射出的类型名建表：映射若与 PostgreSQL 类型名漂移，
    // 这里会在真实实例上失败。
    let ddl = format!(
        "CREATE TABLE phase_b_it.networks (
            id BIGSERIAL PRIMARY KEY,
            ip {},
            net {},
            mac {},
            mac8 {},
            fixed_bits {}(8),
            var_bits {},
            aliases {}
        )",
        type_name(FieldType::Inet),
        type_name(FieldType::Cidr),
        type_name(FieldType::MacAddr),
        type_name(FieldType::MacAddr8),
        type_name(FieldType::Bit),
        type_name(FieldType::Varbit),
        type_name(FieldType::Array(Box::new(FieldType::Inet))),
    );
    db.execute_with_params(&ddl, vec![])
        .await
        .expect("按 FieldType 映射建表应成功");

    db.execute_with_params(
        "INSERT INTO phase_b_it.networks
            (ip, net, mac, mac8, fixed_bits, var_bits, aliases)
         VALUES
            ('192.168.1.10', '10.0.0.0/8', '08:00:2b:01:02:03',
             '08:00:2b:01:02:03:04:05', B'10101010', B'101',
             ARRAY['10.0.0.1','2001:db8::1']::inet[])",
        vec![],
    )
    .await
    .expect("插入网络类型测试数据应成功");

    type NetworkRow = (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    );
    let rows: Vec<NetworkRow> = db
        .query_with_params(
            "SELECT host(ip), net::text, mac::text, mac8::text,
                    fixed_bits::text, var_bits::text,
                    host(aliases[1]), host(aliases[2])
             FROM phase_b_it.networks
             WHERE ip = $1::inet",
            vec![json!("192.168.1.10")],
        )
        .await
        .expect("INET 系列类型查询应成功");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.0, "192.168.1.10");
    assert_eq!(row.1, "10.0.0.0/8");
    assert_eq!(row.2, "08:00:2b:01:02:03");
    assert_eq!(row.3, "08:00:2b:01:02:03:04:05");
    assert_eq!(row.4, "10101010");
    assert_eq!(row.5, "101");
    assert_eq!(row.6, "10.0.0.1");
    assert_eq!(row.7, "2001:db8::1");

    drop_schema(&db).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_bytea_roundtrip() {
    let db = connect(&test_db_url()).await;
    prepare_schema(&db).await;
    db.execute_with_params("DROP TABLE IF EXISTS phase_b_it.blobs", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_it.blobs (id BIGSERIAL PRIMARY KEY, payload BYTEA NOT NULL)",
        vec![],
    )
    .await
    .expect("创建 BYTEA 测试表应成功");

    // 覆盖全部字节值（含 0x00 / 0xFF），确保二进制传输没有被文本化截断。
    let payload: Vec<u8> = (0..=255_u8).collect();
    let encoded = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&payload)
    };
    let id = db
        .table(table!("phase_b_it.blobs"))
        .blob(field!("payload"))
        .insert(&json!({"payload": encoded}))
        .await
        .expect("BYTEA 插入应成功");
    assert!(id > 0);

    #[derive(Debug, sqlx::FromRow)]
    struct BlobRow {
        payload: Vec<u8>,
    }

    // 解码侧：sqlx 原生把 BYTEA 解码为 Vec<u8>。
    let rows: Vec<BlobRow> = db
        .table(table!("phase_b_it.blobs"))
        // 绑定侧：SqlValue::Bytes → BYTEA 参数，与服务端存储字节逐字节相等。
        .where_and(
            field!("payload"),
            CompareOp::Eq,
            SqlValue::Bytes(payload.clone()),
        )
        .expect("受控条件应合法")
        .field(field!("payload"))
        .select::<BlobRow>()
        .await
        .expect("BYTEA 查询应成功");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].payload, payload);

    drop_schema(&db).await;
}

#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_session_guc_baseline_is_applicable() {
    let db = connect(&test_db_url()).await;
    // 事务固定同一条连接，保证 set_config 与 SHOW 看到同一会话。
    let mut tx = db.transaction().await.expect("开启事务应成功");
    for (name, value) in SESSION_GUC_BASELINE {
        let applied: Vec<(String,)> = tx
            .query_with_params(
                "SELECT set_config($1, $2, false)",
                vec![json!(name), json!(value)],
            )
            .await
            .expect("应用会话 GUC 基线应成功");
        assert_eq!(&applied[0].0, value, "GUC {name} 应被设置为基线值");

        // GUC 名来自本 crate 常量而非用户输入，直接插入 SHOW 语句是安全的。
        let shown: Vec<(String,)> = tx
            .query_with_params(&format!("SHOW {name}"), vec![])
            .await
            .expect("读取会话 GUC 应成功");
        assert_eq!(&shown[0].0, value, "GUC {name} 的会话值应与基线一致");
    }
    tx.rollback().await.expect("回滚事务应成功");
}

/// 网络类型在 WHERE 中必须生成 `$n::inet` 等显式转换：
/// PostgreSQL 没有 text→inet 的隐式赋值转换，裸绑定 text 会报 42883。
#[tokio::test]
#[ignore = "需要真实 PostgreSQL 实例"]
async fn phase_b_network_where_uses_explicit_cast() {
    let db = connect(&test_db_url()).await;
    db.execute_with_params("CREATE SCHEMA IF NOT EXISTS phase_b_types", vec![])
        .await
        .expect("创建 schema 应成功");
    db.execute_with_params("DROP TABLE IF EXISTS phase_b_types.hosts", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        "CREATE TABLE phase_b_types.hosts (
            id BIGSERIAL PRIMARY KEY,
            ip INET NOT NULL,
            mac MACADDR NOT NULL,
            cidr CIDR NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建网络类型表应成功");
    db.execute_with_params(
        "INSERT INTO phase_b_types.hosts (ip, mac, cidr)
         VALUES ('192.168.1.10', '08:00:2b:01:02:03', '192.168.1.0/24')",
        vec![],
    )
    .await
    .expect("插入网络类型行应成功");

    let count = db
        .table(table!("phase_b_types.hosts"))
        .field_type(field!("ip"), FieldType::Inet)
        .field_type(field!("mac"), FieldType::MacAddr)
        .field_type(field!("cidr"), FieldType::Cidr)
        .where_and(field!("ip"), CompareOp::Eq, "192.168.1.10")
        .expect("INET 条件应合法")
        .where_and(field!("mac"), CompareOp::Eq, "08:00:2b:01:02:03")
        .expect("MACADDR 条件应合法")
        .where_and(field!("cidr"), CompareOp::Eq, "192.168.1.0/24")
        .expect("CIDR 条件应合法")
        .count()
        .await
        .expect("网络类型 WHERE 查询应成功");
    assert_eq!(count, 1);

    // IN 列表同样要带显式转换。
    let count = db
        .table(table!("phase_b_types.hosts"))
        .field_type(field!("ip"), FieldType::Inet)
        .where_in(
            field!("ip"),
            vec!["192.168.1.10".to_string(), "10.0.0.1".to_string()],
        )
        .count()
        .await
        .expect("INET IN 查询应成功");
    assert_eq!(count, 1);

    db.execute_with_params("DROP SCHEMA phase_b_types CASCADE", vec![])
        .await
        .expect("清理 schema 应成功");
}
