//! 扫描存量 PostgreSQL schema，输出接入报告与字段声明建议。
//!
//! ```text
//! cargo run -p yang-base --no-default-features --features postgres \
//!   --example pg_adoption_report -- postgres://postgres:postgres@127.0.0.1:55432/test [schema]
//! ```
//!
//! URL 也可以放在 `PG_ADOPTION_URL` 或 `PG_TEST_URL`。本工具只读，不执行 DDL。

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let url = arguments
        .next()
        .or_else(|| std::env::var("PG_ADOPTION_URL").ok())
        .or_else(|| std::env::var("PG_TEST_URL").ok());
    let Some(url) = url else {
        eprintln!(
            "用法: pg_adoption_report <postgres-url> [schema]\n\
             或设置 PG_ADOPTION_URL / PG_TEST_URL"
        );
        std::process::exit(2);
    };
    let schema = arguments.next();
    let database = yang_db::postgres::Database::connect(&url).await?;
    let report =
        yang_base::database::describe_postgres_schema(&database, schema.as_deref()).await?;
    print!("{}", report.to_markdown());
    Ok(())
}
