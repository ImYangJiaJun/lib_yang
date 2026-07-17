//! BR 源码到原生 YANG Interface 的机械迁移工具。

use regex::{Captures, Regex};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const IDENTIFIER: &str = r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*";

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("yang-migrate: {error}");
        std::process::exit(2);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let (apply, paths) = parse_args(args)?;
    let files = collect_rust_files(&paths)?;
    let mut changed = 0usize;
    let mut manual_diagnostics = 0usize;

    for file in files {
        let source = fs::read_to_string(&file)
            .map_err(|error| format!("读取 {} 失败: {error}", file.display()))?;
        for diagnostic in diagnose_source(&source)? {
            manual_diagnostics += 1;
            println!(
                "manual-migration {}:{} {}",
                file.display(),
                diagnostic.line,
                diagnostic.reason
            );
        }
        let converted = convert_source(&source)?;
        if converted == source {
            continue;
        }
        changed += 1;
        if apply {
            fs::write(&file, converted)
                .map_err(|error| format!("写入 {} 失败: {error}", file.display()))?;
            println!("updated {}", file.display());
        } else {
            println!("would-update {}", file.display());
        }
    }

    println!("changed_files={changed} manual_diagnostics={manual_diagnostics} apply={apply}");
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Diagnostic {
    line: usize,
    reason: &'static str,
}

fn diagnose_source(source: &str) -> Result<Vec<Diagnostic>, String> {
    let patterns = [
        (
            r#"\.table\(\s*[A-Za-z_][A-Za-z0-9_]*\s*\)"#,
            "动态表名无法自动转换；请改为受控 table! 字面量或显式分支",
        ),
        (
            r#"\.where_(?:and|or)\(\s*[A-Za-z_][A-Za-z0-9_]*\s*,"#,
            "动态字段名无法自动转换；请改为受控 field! 字面量或类型化条件",
        ),
        (
            r#"request\.body\s*\["#,
            "动态 JsonValue 参数读取无法自动转换；请声明 params! 强类型 Input",
        ),
        (
            r#"Plugins::api_run\s*\("#,
            "字符串内部调用无法自动转换；请声明 ActionLink 并通过 ctx.plugins() 调用",
        ),
        (
            r#"br_fields::"#,
            "BR 字段 Builder 需要迁移到 fields!/params! 中的 YANG Builder",
        ),
        (
            r#"self\.tools\(\)"#,
            "Action 资源所有权需要显式迁移为 ctx.tools()",
        ),
    ];
    let compiled = patterns
        .into_iter()
        .map(|(pattern, reason)| {
            Regex::new(pattern)
                .map(|regex| (regex, reason))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut diagnostics = Vec::new();
    for (index, line) in source.lines().enumerate() {
        for (regex, reason) in &compiled {
            if regex.is_match(line) {
                diagnostics.push(Diagnostic {
                    line: index + 1,
                    reason,
                });
            }
        }
    }
    Ok(diagnostics)
}

fn parse_args(args: Vec<String>) -> Result<(bool, Vec<PathBuf>), String> {
    let mut apply = false;
    let mut paths = Vec::new();
    for arg in args {
        match arg.as_str() {
            "br-to-yang" => {}
            "--apply" => apply = true,
            "--check" => apply = false,
            "--help" | "-h" => {
                return Err(
                    "用法: cargo run -p yang-migrate -- br-to-yang [--check|--apply] <path>..."
                        .to_string(),
                );
            }
            value if value.starts_with('-') => return Err(format!("未知参数: {value}")),
            value => paths.push(PathBuf::from(value)),
        }
    }
    if paths.is_empty() {
        return Err("至少需要一个源码路径".to_string());
    }
    Ok((apply, paths))
}

fn collect_rust_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for path in paths {
        collect_path(path, &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_path(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    if !path.is_dir() {
        return Err(format!("路径不存在: {}", path.display()));
    }
    for entry in
        fs::read_dir(path).map_err(|error| format!("读取目录 {} 失败: {error}", path.display()))?
    {
        let entry = entry.map_err(|error| format!("读取目录项失败: {error}"))?;
        collect_path(&entry.path(), files)?;
    }
    Ok(())
}

fn convert_source(source: &str) -> Result<String, String> {
    let mut result = source.to_string();

    let table_binding = Regex::new(&format!(
        r#"(?m)let\s+(table|table_name)\s*=\s*"({IDENTIFIER})"\s*;"#
    ))
    .map_err(|error| error.to_string())?;
    result = table_binding
        .replace_all(&result, |captures: &Captures<'_>| {
            format!(
                "let {} = yang_db::table!(\"{}\");",
                &captures[1], &captures[2]
            )
        })
        .into_owned();

    result = replace_identifier_call(&result, "table", "table", "yang_db::table!")?;
    result = replace_identifier_call(&result, "drop_table", "drop_table", "yang_db::table!")?;
    result = replace_identifier_call(&result, "table_exists", "table_exists", "yang_db::table!")?;
    result = replace_identifier_call(&result, "field_identifier", "field", "yang_db::field!")?;
    result = replace_identifier_call(&result, "field", "field", "yang_db::field!")?;

    for (source_method, target_method) in [
        ("where_and", "where_and"),
        ("where_and_unchecked", "where_and"),
        ("where_or", "where_or"),
        ("where_or_unchecked", "where_or"),
        ("having_cond", "having_cond"),
        ("having_cond_unchecked", "having_cond"),
    ] {
        for (operator, variant) in [
            (">=", "Gte"),
            ("<=", "Lte"),
            ("!=", "Ne"),
            ("=", "Eq"),
            (">", "Gt"),
            ("<", "Lt"),
            ("like", "Like"),
            ("LIKE", "Like"),
        ] {
            let pattern = Regex::new(&format!(
                r#"\.{source_method}\(\s*"({IDENTIFIER})"\s*,\s*"{}"\s*,"#,
                regex::escape(operator)
            ))
            .map_err(|error| error.to_string())?;
            result = pattern
                .replace_all(&result, |captures: &Captures<'_>| {
                    format!(
                        ".{target_method}(yang_db::field!(\"{}\"), yang_db::CompareOp::{variant},",
                        &captures[1]
                    )
                })
                .into_owned();
        }
    }

    for method in [
        "json",
        "datetime",
        "timestamp",
        "decimal",
        "blob",
        "text",
        "group",
        "group_identifier",
        "where_null",
        "where_not_null",
        "where_in",
        "where_in_subquery",
        "where_between",
        "value",
        "sum",
        "avg",
        "min",
        "max",
        "increment",
        "decrement",
        "returning",
    ] {
        let target = if method == "group_identifier" {
            "group"
        } else {
            method
        };
        let pattern = Regex::new(&format!(r#"\.{method}\(\s*"({IDENTIFIER})""#))
            .map_err(|error| error.to_string())?;
        result = pattern
            .replace_all(&result, |captures: &Captures<'_>| {
                format!(".{target}(yang_db::field!(\"{}\")", &captures[1])
            })
            .into_owned();
    }

    let fields_call = Regex::new(&format!(
        r#"\.fields\(\s*&\[((?:\s*"{IDENTIFIER}"\s*,?)+)\]\s*\)"#
    ))
    .map_err(|error| error.to_string())?;
    let quoted_identifier =
        Regex::new(&format!(r#""({IDENTIFIER})""#)).map_err(|error| error.to_string())?;
    result = fields_call
        .replace_all(&result, |captures: &Captures<'_>| {
            let fields = quoted_identifier
                .captures_iter(&captures[1])
                .map(|field| format!("yang_db::field!(\"{}\")", &field[1]))
                .collect::<Vec<_>>()
                .join(", ");
            format!(".fields(&[{fields}])")
        })
        .into_owned();

    for (ascending, variant) in [(true, "Asc"), (false, "Desc")] {
        for method in ["order", "order_identifier"] {
            let pattern = Regex::new(&format!(
                r#"\.{method}\(\s*"({IDENTIFIER})"\s*,\s*{ascending}\s*\)"#
            ))
            .map_err(|error| error.to_string())?;
            result = pattern
                .replace_all(&result, |captures: &Captures<'_>| {
                    format!(
                        ".order(yang_db::field!(\"{}\"), yang_db::SortOrder::{variant})",
                        &captures[1]
                    )
                })
                .into_owned();
        }
    }

    let conflict_list = Regex::new(&format!(
        r#"\.on_conflict\(\s*&\[\s*"({IDENTIFIER})"\s*\]\s*\)"#
    ))
    .map_err(|error| error.to_string())?;
    result = conflict_list
        .replace_all(&result, |captures: &Captures<'_>| {
            format!(".on_conflict(&[yang_db::field!(\"{}\")])", &captures[1])
        })
        .into_owned();

    for (source_method, target_method) in [
        ("join", "join"),
        ("join_on_identifiers", "join"),
        ("left_join", "left_join"),
        ("right_join", "right_join"),
    ] {
        let pattern = Regex::new(&format!(
            r#"\.{source_method}\(\s*"({IDENTIFIER})"\s*,\s*"({IDENTIFIER})\s*=\s*({IDENTIFIER})"\s*\)"#
        ))
        .map_err(|error| error.to_string())?;
        result = pattern
            .replace_all(&result, |captures: &Captures<'_>| {
                format!(
                    ".{target_method}(yang_db::table!(\"{}\"), yang_db::field!(\"{}\"), yang_db::field!(\"{}\"))",
                    &captures[1], &captures[2], &captures[3]
                )
            })
            .into_owned();

        let three_argument_pattern = Regex::new(&format!(
            r#"\.{source_method}\(\s*"({IDENTIFIER})"\s*,\s*"({IDENTIFIER})"\s*,\s*"({IDENTIFIER})"\s*\)"#
        ))
        .map_err(|error| error.to_string())?;
        result = three_argument_pattern
            .replace_all(&result, |captures: &Captures<'_>| {
                format!(
                    ".{target_method}(yang_db::table!(\"{}\"), yang_db::field!(\"{}\"), yang_db::field!(\"{}\"))",
                    &captures[1], &captures[2], &captures[3]
                )
            })
            .into_owned();
    }

    // 旧 checked builder 返回 Result；新受控引用在宏展开时已经校验，调用本身不可失败。
    let obsolete_terminal = Regex::new(
        r#"(?m)(\.(?:field|group|order|where_and|where_or|having_cond)\([^\r\n]+\))\s*\.(?:expect\([^\r\n]*\)|unwrap\(\))"#,
    )
    .map_err(|error| error.to_string())?;
    result = obsolete_terminal.replace_all(&result, "$1").into_owned();
    let obsolete_question =
        Regex::new(r#"(?m)(\.(?:field|group|order|where_and|where_or|having_cond)\([^\r\n]+\))\?"#)
            .map_err(|error| error.to_string())?;
    result = obsolete_question.replace_all(&result, "$1").into_owned();

    Ok(result)
}

fn replace_identifier_call(
    source: &str,
    source_method: &str,
    target_method: &str,
    macro_path: &str,
) -> Result<String, String> {
    let pattern = Regex::new(&format!(r#"\.{source_method}\(\s*"({IDENTIFIER})"\s*\)"#))
        .map_err(|error| error.to_string())?;
    Ok(pattern
        .replace_all(source, |captures: &Captures<'_>| {
            format!(".{target_method}({macro_path}(\"{}\"))", &captures[1])
        })
        .into_owned())
}

#[cfg(test)]
mod tests {
    use super::{convert_source, diagnose_source};

    #[test]
    fn converts_controlled_query_literals() {
        let source = r#"
db.table("users")
    .field_identifier("users.id")
    .where_and("users.status", "=", 1)
    .order("users.created_at", false)
    .sum("users.amount");
"#;
        let converted = convert_source(source).expect("codemod 应成功");
        assert!(converted.contains(".table(yang_db::table!(\"users\"))"));
        assert!(converted.contains(".field(yang_db::field!(\"users.id\"))"));
        assert!(converted
            .contains(".where_and(yang_db::field!(\"users.status\"), yang_db::CompareOp::Eq,"));
        assert!(converted
            .contains(".order(yang_db::field!(\"users.created_at\"), yang_db::SortOrder::Desc)"));
        assert!(converted.contains(".sum(yang_db::field!(\"users.amount\"))"));
    }

    #[test]
    fn leaves_dynamic_names_for_manual_migration() {
        let source = "db.table(table_name).where_and(field_name, op, value);";
        assert_eq!(convert_source(source).expect("codemod 应成功"), source);
        let diagnostics = diagnose_source(source).expect("诊断应成功");
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].reason.contains("动态表名"));
        assert!(diagnostics[1].reason.contains("动态字段名"));
    }

    #[test]
    fn diagnoses_dynamic_br_action_paths_with_source_lines() {
        let source = r#"
let name = request.body["name"].clone();
let result = Plugins::api_run("org.user.register", request)?;
self.tools().db.table(table_name);
"#;
        let diagnostics = diagnose_source(source).expect("诊断应成功");
        assert_eq!(diagnostics.len(), 4);
        assert_eq!(diagnostics[0].line, 2);
        assert!(diagnostics
            .iter()
            .any(|item| item.reason.contains("params!")));
        assert!(diagnostics
            .iter()
            .any(|item| item.reason.contains("ActionLink")));
        assert!(diagnostics
            .iter()
            .any(|item| item.reason.contains("ctx.tools()")));
    }

    #[test]
    fn converts_join_and_removes_obsolete_builder_result_handling() {
        let source = r#"
db.table("users")
    .field_identifier("users.id")
    .expect("合法字段")
    .join("orders", "users.id = orders.user_id")
    .where_and("users.id", "=", 1)?;
"#;
        let converted = convert_source(source).expect("codemod 应成功");
        assert!(converted.contains(
            ".join(yang_db::table!(\"orders\"), yang_db::field!(\"users.id\"), yang_db::field!(\"orders.user_id\"))"
        ));
        assert!(!converted.contains("expect(\"合法字段\")"));
        assert!(!converted.contains("CompareOp::Eq, 1)?"));
    }
}
