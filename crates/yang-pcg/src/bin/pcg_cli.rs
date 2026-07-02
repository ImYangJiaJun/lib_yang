//! `pcg_cli` —— 命令行地图生成工具（UE5 集成路线 B）
//!
//! 供 UE5 在运行时通过 `FPlatformProcess::CreateProc` 调用：传入种子与配置，
//! 生成一张地图并写入文件（JSON 或 `.ypcg` 二进制），UE5 侧再读回构建关卡。
//!
//! # 用法
//!
//! ```bash
//! pcg_cli --seed 12345 --out floor.json
//! pcg_cli --seed 12345 --format binary --out floor.ypcg
//! pcg_cli --config my_config.json --out floor.json
//! pcg_cli --help
//! ```
//!
//! # 选项
//!
//! - `--seed <u64>`       随机种子。省略时从配置派生确定性种子（相同配置复现同图）。
//! - `--config <path>`    配置 JSON 文件路径。省略时用默认配置。
//! - `--out <path>`       输出文件路径（必填）。
//! - `--format <fmt>`     输出格式：`json`（默认）| `compact` | `binary`。
//! - `--trace-id <str>`   追踪标识，写入结果元数据。
//! - `-h` / `--help`      打印帮助。
//!
//! # 退出码
//!
//! - `0` 成功
//! - `1` 参数错误（缺 `--out`、未知参数、缺参数值等）
//! - `2` 配置文件读取/解析失败
//! - `3` 地图生成失败（含硬校验失败）
//! - `4` 输出写入失败

use std::process::ExitCode;

use yang_pcg::{
    export_binary, export_json, export_json_compact, GenerationConfig, GenerationRequest,
    MapGenerator,
};

/// 输出格式。
#[derive(Debug)]
enum OutputFormat {
    Json,
    Compact,
    Binary,
}

/// 解析后的命令行参数。
#[derive(Debug)]
struct CliArgs {
    seed: Option<u64>,
    config_path: Option<String>,
    out_path: String,
    format: OutputFormat,
    trace_id: Option<String>,
}

const HELP: &str = "\
pcg_cli —— 地图生成命令行工具（UE5 集成路线 B）

用法:
  pcg_cli --out <path> [--seed <u64>] [--config <path>] [--format <fmt>] [--trace-id <str>]

选项:
  --seed <u64>       随机种子。省略时从配置派生确定性种子（相同配置复现同图）。
  --config <path>    配置 JSON 文件路径。省略时用默认配置。
  --out <path>       输出文件路径（必填）。
  --format <fmt>     输出格式: json（默认）| compact | binary。
  --trace-id <str>   追踪标识，写入结果元数据。
  -h, --help         打印本帮助。

退出码:
  0 成功 | 1 参数错误 | 2 配置读取失败 | 3 生成失败 | 4 写入失败

示例:
  pcg_cli --seed 12345 --out floor.json
  pcg_cli --seed 12345 --format binary --out floor.ypcg
  pcg_cli --config dungeon.json --out floor.json";

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    // 帮助优先
    if raw.iter().any(|a| a == "-h" || a == "--help") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }

    let args = match parse_args(&raw) {
        Ok(args) => args,
        Err(msg) => {
            eprintln!("参数错误: {msg}\n\n{HELP}");
            return ExitCode::from(1);
        }
    };

    // 加载配置
    let config = match load_config(args.config_path.as_deref()) {
        Ok(config) => config,
        Err(msg) => {
            eprintln!("配置错误: {msg}");
            return ExitCode::from(2);
        }
    };

    // 生成
    let mut request = GenerationRequest::new(config);
    if let Some(seed) = args.seed {
        request = request.with_seed(seed);
    }
    if let Some(trace_id) = args.trace_id {
        request = request.with_trace_id(trace_id);
    }
    let result = match MapGenerator::new().generate(request) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("生成失败: {e}");
            return ExitCode::from(3);
        }
    };

    // 序列化
    let bytes: Vec<u8> = match args.format {
        OutputFormat::Json => match export_json(&result) {
            Ok(s) => s.into_bytes(),
            Err(e) => {
                eprintln!("序列化失败: {e}");
                return ExitCode::from(4);
            }
        },
        OutputFormat::Compact => match export_json_compact(&result) {
            Ok(s) => s.into_bytes(),
            Err(e) => {
                eprintln!("序列化失败: {e}");
                return ExitCode::from(4);
            }
        },
        OutputFormat::Binary => match export_binary(&result) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("序列化失败: {e}");
                return ExitCode::from(4);
            }
        },
    };

    // 写盘
    if let Err(e) = std::fs::write(&args.out_path, &bytes) {
        eprintln!("写入 {} 失败: {e}", args.out_path);
        return ExitCode::from(4);
    }

    // 成功摘要写到 stdout，供调用方解析
    println!(
        "ok seed={} rooms={} items={} enemies={} bytes={} out={}",
        result.metadata.seed,
        result.rooms.len(),
        result.item_spawns.len(),
        result.enemy_spawns.len(),
        bytes.len(),
        args.out_path,
    );
    ExitCode::SUCCESS
}

/// 解析命令行参数。
fn parse_args(raw: &[String]) -> Result<CliArgs, String> {
    let mut seed = None;
    let mut config_path = None;
    let mut out_path = None;
    let mut format = OutputFormat::Json;
    let mut trace_id = None;

    let mut i = 0;
    while i < raw.len() {
        let arg = raw[i].as_str();
        match arg {
            "--seed" => {
                let v = next_value(raw, &mut i, "--seed")?;
                seed = Some(
                    v.parse::<u64>()
                        .map_err(|_| format!("--seed 需要 u64，得到 '{v}'"))?,
                );
            }
            "--config" => {
                config_path = Some(next_value(raw, &mut i, "--config")?);
            }
            "--out" => {
                out_path = Some(next_value(raw, &mut i, "--out")?);
            }
            "--format" => {
                let v = next_value(raw, &mut i, "--format")?;
                format = match v.as_str() {
                    "json" => OutputFormat::Json,
                    "compact" => OutputFormat::Compact,
                    "binary" => OutputFormat::Binary,
                    other => return Err(format!("未知 --format '{other}'（json|compact|binary）")),
                };
            }
            "--trace-id" => {
                trace_id = Some(next_value(raw, &mut i, "--trace-id")?);
            }
            other => return Err(format!("未知参数 '{other}'")),
        }
        i += 1;
    }

    let out_path = out_path.ok_or_else(|| "缺少必填参数 --out".to_string())?;
    Ok(CliArgs {
        seed,
        config_path,
        out_path,
        format,
        trace_id,
    })
}

/// 读取 `--key value` 形式的下一个值，并推进游标。
fn next_value(raw: &[String], i: &mut usize, key: &str) -> Result<String, String> {
    *i += 1;
    raw.get(*i)
        .cloned()
        .ok_or_else(|| format!("{key} 缺少参数值"))
}

/// 加载配置：给定路径则从 JSON 文件读取，否则用默认配置。
///
/// # 安全
///
/// 路径穿越防护：将用户提供的路径 canonicalize 后校验是否仍在当前工作目录范围内。
fn load_config(path: Option<&str>) -> Result<GenerationConfig, String> {
    match path {
        None => Ok(GenerationConfig::default()),
        Some(path) => {
            let user_path = std::path::Path::new(path);
            let canonical = user_path
                .canonicalize()
                .map_err(|e| format!("路径 {path} 无法解析: {e}"))?;
            let allowed_base = std::env::current_dir()
                .map_err(|e| format!("获取当前工作目录失败: {e}"))?
                .canonicalize()
                .map_err(|e| format!("规范化工作目录失败: {e}"))?;
            if !canonical.starts_with(&allowed_base) {
                return Err(format!(
                    "路径 {path} 超出允许范围（允许目录: {}）",
                    allowed_base.display()
                ));
            }
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("读取 {path} 失败: {e}"))?;
            serde_json::from_str::<GenerationConfig>(&text)
                .map_err(|e| format!("解析 {path} 失败: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── parse_args 正常路径 ───

    #[test]
    fn parse_args_minimal() {
        let raw: Vec<String> = vec!["--out", "floor.json"]
            .into_iter()
            .map(String::from)
            .collect();
        let args = parse_args(&raw).expect("最小参数应解析成功");
        assert_eq!(args.out_path, "floor.json");
        assert!(args.seed.is_none());
        assert!(args.config_path.is_none());
        assert!(args.trace_id.is_none());
    }

    #[test]
    fn parse_args_full_options() {
        let raw: Vec<String> = vec![
            "--seed", "42", "--config", "cfg.json", "--out", "out.json",
            "--format", "binary", "--trace-id", "run-001",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let args = parse_args(&raw).expect("全参数应解析成功");
        assert_eq!(args.seed, Some(42));
        assert_eq!(args.config_path.as_deref(), Some("cfg.json"));
        assert_eq!(args.out_path, "out.json");
        assert!(matches!(args.format, OutputFormat::Binary));
        assert_eq!(args.trace_id.as_deref(), Some("run-001"));
    }

    #[test]
    fn parse_args_format_compact() {
        let raw: Vec<String> = vec!["--out", "x.json", "--format", "compact"]
            .into_iter()
            .map(String::from)
            .collect();
        let args = parse_args(&raw).expect("compact 格式应解析成功");
        assert!(matches!(args.format, OutputFormat::Compact));
    }

    // ─── parse_args 错误路径 ───

    #[test]
    fn parse_args_missing_out() {
        let raw: Vec<String> = vec!["--seed", "42"]
            .into_iter()
            .map(String::from)
            .collect();
        let err = parse_args(&raw).expect_err("缺 --out 应报错");
        assert!(err.contains("--out"), "错误信息应提及 --out");
    }

    #[test]
    fn parse_args_unknown_flag() {
        let raw: Vec<String> = vec!["--out", "x.json", "--unknown-flag"]
            .into_iter()
            .map(String::from)
            .collect();
        let err = parse_args(&raw).expect_err("未知参数应报错");
        assert!(err.contains("未知参数"), "错误信息应提及未知参数");
    }

    #[test]
    fn parse_args_seed_not_u64() {
        let raw: Vec<String> = vec!["--out", "x.json", "--seed", "abc"]
            .into_iter()
            .map(String::from)
            .collect();
        let err = parse_args(&raw).expect_err("非 u64 seed 应报错");
        assert!(err.contains("u64"), "错误信息应提及 u64");
    }

    #[test]
    fn parse_args_unknown_format() {
        let raw: Vec<String> = vec!["--out", "x.json", "--format", "xml"]
            .into_iter()
            .map(String::from)
            .collect();
        let err = parse_args(&raw).expect_err("未知 format 应报错");
        assert!(err.contains("format"), "错误信息应提及 format");
    }

    #[test]
    fn parse_args_seed_missing_value() {
        let raw: Vec<String> = vec!["--out", "x.json", "--seed"]
            .into_iter()
            .map(String::from)
            .collect();
        let err = parse_args(&raw).expect_err("--seed 缺值应报错");
        assert!(
            err.contains("缺少参数值"),
            "错误信息应提及缺少参数值"
        );
    }

    // ─── load_config 测试 ───

    #[test]
    fn load_config_none_returns_default() {
        let config = load_config(None).expect("None 应返回默认配置");
        let default = yang_pcg::GenerationConfig::default();
        assert_eq!(
            config.room_count.min, default.room_count.min,
            "默认配置 room_count.min 应一致"
        );
        assert_eq!(
            config.room_count.max, default.room_count.max,
            "默认配置 room_count.max 应一致"
        );
    }

    #[test]
    fn load_config_nonexistent_file() {
        let err = load_config(Some("nonexistent_file_12345.json"))
            .expect_err("不存在的文件应报错");
        assert!(
            err.contains("无法解析") || err.contains("读取"),
            "错误信息应包含路径错误描述: {}",
            err
        );
    }

    #[test]
    fn load_config_invalid_json() {
        // 必须在当前工作目录下创建临时文件，否则 load_config 的路径穿越防护会先拦截
        let dir = std::env::current_dir()
            .expect("获取 CWD")
            .join("__pcg_cli_test_invalid_json");
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        let file_path = dir.join("bad.json");
        std::fs::write(&file_path, "{not valid json!!!").expect("写临时文件应成功");

        let err = load_config(Some(file_path.to_str().unwrap()))
            .expect_err("非法 JSON 应报错");
        assert!(
            err.contains("解析"),
            "错误信息应提及解析失败: {}",
            err
        );

        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir(&dir);
    }
}
