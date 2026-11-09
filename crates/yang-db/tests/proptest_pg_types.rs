#![allow(clippy::expect_used, clippy::unwrap_used)]
//! PostgreSQL 类型属性测试：UUID / NUMERIC / TIMESTAMPTZ / 数组 / JSONB 的
//! 写入-读取往返（proptest 生成输入，真实数据库执行）。
//!
//! 测试标记 `#[ignore]`，默认离线套件不触库。运行方式：
//!
//! ```text
//! PG_TEST_URL=postgres://postgres:postgres@127.0.0.1:55432/test \
//!   cargo test -p yang-db --no-default-features --features postgres \
//!   --test proptest_pg_types -- --ignored --test-threads=1
//! ```
//!
//! 覆盖点：驱动绑定与 sqlx 解码必须逐值等价，而不是“能存进去、看起来差不多”。
//! - UUID：任意 16 字节。
//! - NUMERIC：任意 u64 尾数 + 0..=9 小数位，按精确十进制数值比较，不经过 f64
//!   （PostgreSQL 不保留尾随零，故不比较字符串 scale）。
//! - TIMESTAMPTZ：任意微秒精度时刻，按 UTC 绝对时间比较。
//! - 数组：任意 TEXT[]（含空数组）。
//! - JSONB：标量/数组/对象三层组合。

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use proptest::prelude::*;
use proptest::strategy::Just;
use proptest::test_runner::{Config, TestRunner};
use serde_json::{Map, Value};
use yang_db::postgres::{Database, FieldType};
use yang_db::{field, table};

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
    db.execute_with_params("CREATE SCHEMA IF NOT EXISTS proptest_pg_types", vec![])
        .await
        .expect("创建属性测试 schema 应成功");
    db.execute_with_params("DROP TABLE IF EXISTS proptest_pg_types.round_trips", vec![])
        .await
        .expect("删除旧表应成功");
    db.execute_with_params(
        "CREATE TABLE proptest_pg_types.round_trips (
            id BIGSERIAL PRIMARY KEY,
            external_id UUID NOT NULL,
            amount NUMERIC NOT NULL,
            happened_at TIMESTAMPTZ NOT NULL,
            tags TEXT[] NOT NULL,
            payload JSONB NOT NULL
        )",
        vec![],
    )
    .await
    .expect("创建属性测试表应成功");
}

async fn drop_schema(db: &Database) {
    db.execute_with_params("DROP SCHEMA proptest_pg_types CASCADE", vec![])
        .await
        .expect("清理属性测试 schema 应成功");
}

#[derive(Debug, sqlx::FromRow)]
struct RoundTripRow {
    external_id: uuid::Uuid,
    amount: BigDecimal,
    happened_at: DateTime<Utc>,
    tags: Vec<String>,
    payload: Value,
}

/// PostgreSQL TEXT / JSON 字符串不接受 `\0`，生成策略必须排除。
fn safe_text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        any::<char>().prop_filter("PostgreSQL 文本不接受 NUL", |c| *c != '\0'),
        0..12,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

/// NUMERIC 的精确十进制文本：`mantissa` 配合 `scale` 位小数。
fn decimal_text(mantissa: u64, scale: u32) -> String {
    if scale == 0 {
        mantissa.to_string()
    } else {
        let divisor = 10_u64.pow(scale);
        format!(
            "{}.{:0width$}",
            mantissa / divisor,
            mantissa % divisor,
            width = scale as usize
        )
    }
}

/// JSONB 值：标量 / 数组 / 扁平对象，避免无限递归。
fn json_value() -> impl Strategy<Value = Value> {
    let scalar = prop_oneof![
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        safe_text().prop_map(Value::String),
        Just(Value::Null),
    ];
    prop_oneof![
        scalar.clone(),
        prop::collection::vec(scalar.clone(), 0..4).prop_map(Value::Array),
        prop::collection::vec((0_usize..4, scalar), 0..4).prop_map(|entries| {
            let mut object = Map::new();
            for (index, value) in entries {
                object.insert(format!("k{index}"), value);
            }
            Value::Object(object)
        }),
    ]
}

fn round_trip_case() -> impl Strategy<Value = ([u8; 16], String, DateTime<Utc>, Vec<String>, Value)>
{
    (
        any::<[u8; 16]>(),
        (any::<u64>(), 0_u32..=9).prop_map(|(mantissa, scale)| decimal_text(mantissa, scale)),
        (-2_000_000_000_i64..2_000_000_000, 0_u32..1_000_000),
        prop::collection::vec(safe_text(), 0..8),
        json_value(),
    )
        .prop_map(|(uuid_bytes, amount, (seconds, micros), tags, payload)| {
            // from_timestamp 第二参数是纳秒；PostgreSQL 只存微秒，按微秒生成。
            let happened_at = DateTime::<Utc>::from_timestamp(seconds, micros * 1_000)
                .expect("测试生成的时间戳必须在 chrono 可表示范围内");
            (uuid_bytes, amount, happened_at, tags, payload)
        })
}

#[test]
#[ignore = "需要真实 PostgreSQL 实例"]
fn proptest_pg_types_write_read_round_trip() {
    let runtime = tokio::runtime::Runtime::new().expect("创建 Tokio 运行时应成功");
    let db = runtime.block_on(connect(&test_db_url()));
    runtime.block_on(prepare_schema(&db));

    let mut runner = TestRunner::new(Config {
        // 真库往返，控制用例数以兼顾覆盖和 CI 时长。
        cases: 48,
        ..Config::default()
    });

    runner
        .run(
            &round_trip_case(),
            |(uuid_bytes, amount, happened_at, tags, payload)| {
                let external_id = uuid::Uuid::from_bytes(uuid_bytes);
                let inserted: RoundTripRow = runtime
                    .block_on(async {
                        db.table(table!("proptest_pg_types.round_trips"))
                            .uuid(field!("external_id"))
                            .decimal(field!("amount"))
                            .timestamp_tz(field!("happened_at"))
                            .field_type(field!("tags"), FieldType::Array(Box::new(FieldType::Text)))
                            .jsonb(field!("payload"))
                            .returning_all()
                            .insert_returning::<_, RoundTripRow>(&serde_json::json!({
                                "external_id": external_id.to_string(),
                                "amount": amount,
                                "happened_at": happened_at.to_rfc3339(),
                                "tags": tags,
                                "payload": payload,
                            }))
                            .await
                    })
                    .expect("属性输入应能完成写入并 RETURNING 解码");

                // 写入路径的 RETURNING 解码结果必须逐值相等。
                prop_assert_eq!(&inserted.external_id, &external_id);
                // PostgreSQL NUMERIC 不保留尾随零（`0.0` 读回 `0`），按数值比较。
                prop_assert_eq!(
                    &inserted.amount,
                    &amount
                        .parse::<BigDecimal>()
                        .expect("生成的十进制文本必须可解析"),
                    "NUMERIC 必须保持精确十进制数值"
                );
                prop_assert_eq!(inserted.happened_at, happened_at);
                prop_assert_eq!(&inserted.tags, &tags);
                prop_assert_eq!(&inserted.payload, &payload);

                // 独立查询路径：重新读出同一行，验证不是仅 RETURNING 恰好正确。
                let read_back: Vec<RoundTripRow> = runtime
                    .block_on(async {
                        db.table(table!("proptest_pg_types.round_trips"))
                            .where_and(field!("external_id"), yang_db::CompareOp::Eq, external_id)
                            .expect("UUID 等值条件应合法")
                            .order(field!("id"), yang_db::SortOrder::Desc)
                            .limit(1)
                            .select::<RoundTripRow>()
                            .await
                    })
                    .expect("属性输入应能按 UUID 重新读出");

                prop_assert_eq!(read_back.len(), 1);
                let read_back = &read_back[0];
                prop_assert_eq!(&read_back.external_id, &external_id);
                prop_assert_eq!(&read_back.amount, &inserted.amount);
                prop_assert_eq!(read_back.happened_at, happened_at);
                prop_assert_eq!(&read_back.tags, &tags);
                prop_assert_eq!(&read_back.payload, &payload);
                Ok(())
            },
        )
        .expect("PostgreSQL 类型属性测试失败");

    runtime.block_on(drop_schema(&db));
}
