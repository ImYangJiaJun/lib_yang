//! 30 行接入样例：只用 crate 公开面拉取一张多维表格。
//!
//! ```bash
//! FEISHU_APP_ID=cli_xxx FEISHU_APP_SECRET=secret \
//! BITABLE_APP_TOKEN=bascn... BITABLE_TABLE_ID=tbl... \
//! cargo run --example bitable_pull -p yang-feishu --features http
//! ```

use std::sync::Arc;

use yang_feishu::auth::MemoryTokenCache;
use yang_feishu::transport::TokioSleeper;
use yang_feishu::{Endpoints, OpenApiClient, OpenApiConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenApiClient::builder(
        OpenApiConfig::static_credentials(
            std::env::var("FEISHU_APP_ID")?,
            std::env::var("FEISHU_APP_SECRET")?,
        )
        .endpoints(Endpoints::feishu())
        .cache_namespace("demo"),
    )
    .sleeper(Arc::new(TokioSleeper))
    .token_cache(Arc::new(MemoryTokenCache::default()))
    .build()?;

    let snapshot = client
        .bitable()
        .list_all_records(
            &std::env::var("BITABLE_APP_TOKEN")?,
            &std::env::var("BITABLE_TABLE_ID")?,
            None,
            None,
        )
        .await?;
    println!("rows = {}", snapshot.items.len());
    Ok(())
}
