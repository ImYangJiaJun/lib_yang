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
enum OutputFormat {
    Json,
    Compact,
    Binary,
}

/// 解析后的命令行参数。
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
fn load_config(path: Option<&str>) -> Result<GenerationConfig, String> {
    match path {
        None => Ok(GenerationConfig::default()),
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("读取 {path} 失败: {e}"))?;
            serde_json::from_str::<GenerationConfig>(&text)
                .map_err(|e| format!("解析 {path} 失败: {e}"))
        }
    }
}
