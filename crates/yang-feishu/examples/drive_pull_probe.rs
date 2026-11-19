//! 云盘拉取探针：递归列目录并把 `type=file` 下载到本地。
//!
//! ```bash
//! FEISHU_APP_ID=cli_xxx FEISHU_APP_SECRET=secret \
//! FEISHU_FOLDER_TOKEN=fldcn... FEISHU_OUT_DIR=./out \
//! cargo run --example drive_pull_probe -p yang-feishu --features "http,drive"
//! ```

use std::collections::VecDeque;
use std::path::PathBuf;

use yang_feishu::{ApiClient, OpenApiClient, OpenApiConfig};

const MAX_DEPTH: usize = 5;
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenApiClient::builder(OpenApiConfig::static_credentials(
        std::env::var("FEISHU_APP_ID")?,
        std::env::var("FEISHU_APP_SECRET")?,
    ))
    .build()?;
    let root = std::env::var("FEISHU_FOLDER_TOKEN")?;
    let out_dir = PathBuf::from(std::env::var("FEISHU_OUT_DIR")?);

    let files = scan(client.api(), &root).await?;
    println!("== 发现 {} 个可下载文件 ==", files.len());

    let mut downloaded = 0u64;
    let mut total = 0u64;
    for (relative, token) in &files {
        match client.drive().download_file(token, MAX_FILE_BYTES).await {
            Ok(bytes) => {
                let path = out_dir.join(relative);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, &bytes)?;
                downloaded += 1;
                total += bytes.len() as u64;
                println!(
                    "[ok] {:>10} B  {}  token={}",
                    bytes.len(),
                    relative.display(),
                    token
                );
            }
            // 探针继续跑：单个文件失败不影响其他文件。
            Err(error) => println!("[fail] {} token={} : {error}", relative.display(), token),
        }
    }
    println!(
        "== 下载 {downloaded} 个文件，{total} 字节 -> {} ==",
        out_dir.display()
    );
    Ok(())
}

/// 广度优先遍历，返回 `(相对路径, file_token)`。
async fn scan(
    api: ApiClient,
    root: &str,
) -> Result<Vec<(PathBuf, String)>, Box<dyn std::error::Error>> {
    let mut folders = VecDeque::from([(root.to_string(), PathBuf::new(), 0usize)]);
    let mut files = Vec::new();
    while let Some((folder_token, base, depth)) = folders.pop_front() {
        if depth > MAX_DEPTH {
            println!("[skip] 目录深度超过 {MAX_DEPTH}: {}", base.display());
            continue;
        }
        let mut page_token: Option<String> = None;
        loop {
            let mut query: Vec<(&str, &str)> = vec![
                ("folder_token", folder_token.as_str()),
                ("page_size", "200"),
            ];
            if let Some(token) = page_token.as_deref() {
                query.push(("page_token", token));
            }
            let value = api.drive_files_list(&query).await?;
            let data = &value["data"];
            for item in data["files"].as_array().into_iter().flatten() {
                let name = item["name"].as_str().unwrap_or("<unnamed>");
                let token = item["token"].as_str().unwrap_or_default();
                let kind = item["type"].as_str().unwrap_or_default();
                let path = base.join(name);
                println!("[{kind:>8}] {}", path.display());
                match kind {
                    "folder" => folders.push_back((token.to_string(), path, depth + 1)),
                    "file" => files.push((path, token.to_string())),
                    _ => {}
                }
            }
            if !data["has_more"].as_bool().unwrap_or(false) {
                break;
            }
            page_token = data["next_page_token"].as_str().map(str::to_string);
            if page_token.as_deref().unwrap_or_default().is_empty() {
                println!(
                    "[warn] has_more=true 但没有 next_page_token，停止分页: {}",
                    base.display()
                );
                break;
            }
        }
    }
    Ok(files)
}
