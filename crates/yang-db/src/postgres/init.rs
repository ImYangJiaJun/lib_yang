//! PostgreSQL DDL/脚本执行逃生舱（P1-27）。
//!
//! 提供按 PostgreSQL 词法切分多语句脚本的
//! [`split_sql_script`]（识别单/双引号、`E'...'` 转义字符串、行注释、可嵌套块
//! 注释与 `$$`/`$tag$` dollar-quote），以及整脚本单事务执行的
//! [`Database::execute_script`]。函数体、`DO` 块和注释里的分号不会被误切。

use crate::error::DbError;
use crate::postgres::{Database, Transaction};

/// 数据库迁移配置（PostgreSQL）
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// 迁移脚本目录
    pub migrations_dir: String,
    /// 是否自动运行迁移
    pub auto_migrate: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            migrations_dir: "migrations".to_string(),
            auto_migrate: false,
        }
    }
}

/// PostgreSQL 未加引号标识符的首字符规则（`[A-Za-z_]`）。
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

/// PostgreSQL 未加引号标识符的后续字符规则（`[A-Za-z0-9_$]`）。
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// 在 `chars[start] == '$'` 处尝试解析 dollar-quote 标签。
///
/// 返回包含首尾 `$` 的标签（如 `$$`、`$body$`）。标签内容必须为空或符合
/// 未加引号标识符规则（首字符非数字），因此 `$1` 这类位置参数不会被当成标签。
fn parse_dollar_tag(chars: &[char], start: usize) -> Option<Vec<char>> {
    let mut end = start + 1;
    // 标签内容只能是 ASCII 字母/数字/下划线（不含 `$`，`$$` 的标签为空）。
    while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
        end += 1;
    }
    // 标签内首字符不允许是数字（`$1$` 不是合法 dollar-quote 标签）。
    if end > start + 1 && !is_ident_start(chars[start + 1]) {
        return None;
    }
    if end < chars.len() && chars[end] == '$' {
        Some(chars[start..=end].to_vec())
    } else {
        None
    }
}

/// 按 PostgreSQL 词法把一个多语句脚本切分为独立语句。
///
/// 切分规则（P1-27）：
/// - 单引号字符串：`''` 为转义；`E'...'` 前缀下反斜杠转义下一个字符；
/// - 双引号标识符：`""` 为转义；
/// - 行注释 `-- ...` 到行尾；块注释 `/* ... */`，支持嵌套；
/// - dollar-quote `$$...$$` / `$tag$...$tag$`：内部（含函数体分号）原样保留；
/// - 其余位置的分号是语句分隔符。
///
/// 返回去掉首尾空白的语句（不含结尾分号、忽略空语句）。引号/注释/dollar-quote
/// 未闭合时返回 [`DbError::InvalidArgument`]。
pub fn split_sql_script(script: &str) -> Result<Vec<String>, DbError> {
    let chars: Vec<char> = script.chars().collect();
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut i = 0usize;

    while i < chars.len() {
        match chars[i] {
            // 单引号字符串（含 E'...' 反斜杠转义）。
            '\'' => {
                let escape_backslash = i > 0
                    && matches!(chars[i - 1], 'E' | 'e')
                    && (i < 2 || !is_ident_char(chars[i - 2]));
                current.push('\'');
                i += 1;
                let mut closed = false;
                while i < chars.len() {
                    let c = chars[i];
                    current.push(c);
                    if escape_backslash && c == '\\' {
                        i += 1;
                        if i < chars.len() {
                            current.push(chars[i]);
                            i += 1;
                        }
                        continue;
                    }
                    if c == '\'' {
                        if i + 1 < chars.len() && chars[i + 1] == '\'' {
                            current.push('\'');
                            i += 2;
                            continue;
                        }
                        i += 1;
                        closed = true;
                        break;
                    }
                    i += 1;
                }
                if !closed {
                    return Err(unterminated("单引号字符串", script, i));
                }
            }
            // 双引号标识符。
            '"' => {
                current.push('"');
                i += 1;
                let mut closed = false;
                while i < chars.len() {
                    let c = chars[i];
                    current.push(c);
                    if c == '"' {
                        if i + 1 < chars.len() && chars[i + 1] == '"' {
                            current.push('"');
                            i += 2;
                            continue;
                        }
                        i += 1;
                        closed = true;
                        break;
                    }
                    i += 1;
                }
                if !closed {
                    return Err(unterminated("双引号标识符", script, i));
                }
            }
            // 行注释：复制到行尾（不含分隔作用）。
            '-' if chars.get(i + 1) == Some(&'-') => {
                current.push_str("--");
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    current.push(chars[i]);
                    i += 1;
                }
            }
            // 块注释：支持嵌套。
            '/' if chars.get(i + 1) == Some(&'*') => {
                current.push_str("/*");
                i += 2;
                let mut depth = 1usize;
                while i < chars.len() && depth > 0 {
                    if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                        depth += 1;
                        current.push_str("/*");
                        i += 2;
                    } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        current.push_str("*/");
                        i += 2;
                    } else {
                        current.push(chars[i]);
                        i += 1;
                    }
                }
                if depth > 0 {
                    return Err(unterminated("块注释", script, i));
                }
            }
            // dollar-quote：整体复制到匹配的结束标签。
            '$' => match parse_dollar_tag(&chars, i) {
                Some(tag) => {
                    current.extend(tag.iter());
                    i += tag.len();
                    let mut closed = false;
                    while i < chars.len() {
                        if chars.len() - i >= tag.len() && chars[i..i + tag.len()] == tag[..] {
                            current.extend(tag.iter());
                            i += tag.len();
                            closed = true;
                            break;
                        }
                        current.push(chars[i]);
                        i += 1;
                    }
                    if !closed {
                        return Err(unterminated("dollar-quote", script, i));
                    }
                }
                None => {
                    current.push('$');
                    i += 1;
                }
            },
            // 语句分隔符。
            ';' => {
                let statement = current.trim();
                if !statement.is_empty() {
                    statements.push(statement.to_string());
                }
                current.clear();
                i += 1;
            }
            c => {
                current.push(c);
                i += 1;
            }
        }
    }

    let statement = current.trim();
    if !statement.is_empty() {
        statements.push(statement.to_string());
    }
    Ok(statements)
}

/// 构造未闭合词法单元的错误（附带字符位置便于定位）。
fn unterminated(kind: &str, script: &str, position: usize) -> DbError {
    let line = script.chars().take(position).filter(|c| *c == '\n').count() + 1;
    DbError::InvalidArgument(format!(
        "SQL 脚本中存在未闭合的{kind}（第 {line} 行，字符位置 {position}）"
    ))
}

/// 在给定事务内顺序执行已切分的语句（不开启新事务）。
pub(crate) async fn execute_statements(
    transaction: &mut Transaction,
    statements: &[String],
) -> Result<(), DbError> {
    for statement in statements {
        let conn = transaction
            .executor()
            .ok_or_else(|| DbError::TransactionError("事务已提交或回滚".to_string()))?;
        sqlx::query(statement).execute(conn).await?;
    }
    Ok(())
}

impl Database {
    /// 在当前连接池上执行一个多语句脚本（整脚本单事务）。
    ///
    /// 脚本按 [`split_sql_script`] 切分后在同一事务内依次执行；任一条失败则整个
    /// 脚本回滚（DDL 在 PostgreSQL 中可事务化，因此建表失败不会留下半成品）。
    ///
    /// 注意：`CREATE INDEX CONCURRENTLY`、`VACUUM` 等不能在事务块内执行的语句会
    /// 被服务端拒绝；这类语句请单独执行。
    ///
    /// 返回执行的语句条数（空脚本返回 0，且不开事务）。
    pub async fn execute_script(&self, script: &str) -> Result<usize, DbError> {
        let statements = split_sql_script(script)?;
        if statements.is_empty() {
            return Ok(0);
        }
        let mut transaction = self.transaction().await?;
        execute_statements(&mut transaction, &statements).await?;
        transaction.commit().await?;
        Ok(statements.len())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn split(script: &str) -> Vec<String> {
        split_sql_script(script).unwrap()
    }

    #[test]
    fn splits_multiple_statements_and_trims() {
        let statements = split("  SELECT 1; SELECT 2 ;;  SELECT 3  ");
        assert_eq!(statements, vec!["SELECT 1", "SELECT 2", "SELECT 3"]);
        assert!(split("   ; ;  ").is_empty());
        assert!(split("").is_empty());
    }

    #[test]
    fn keeps_semicolons_inside_dollar_quotes() {
        let script = "
            CREATE FUNCTION add_one(x int) RETURNS int AS $$
            BEGIN
                RETURN x + 1;
            END;
            $$ LANGUAGE plpgsql;
            SELECT add_one(1);
        ";
        let statements = split(script);
        assert_eq!(statements.len(), 2);
        assert!(statements[0].contains("RETURN x + 1;"));
        assert!(statements[0].ends_with("LANGUAGE plpgsql"));
        assert_eq!(statements[1], "SELECT add_one(1)");
    }

    #[test]
    fn keeps_semicolons_inside_tagged_dollar_quotes() {
        let script = "DO $body$ BEGIN PERFORM 1; PERFORM 2; END $body$; SELECT 1";
        let statements = split(script);
        assert_eq!(statements.len(), 2);
        assert!(statements[0].starts_with("DO $body$"));
        assert!(statements[0].ends_with("END $body$"));
    }

    #[test]
    fn keeps_semicolons_inside_quotes_and_comments() {
        assert_eq!(
            split("SELECT ';' AS a, 'it''s; ok' AS b; SELECT 2"),
            vec!["SELECT ';' AS a, 'it''s; ok' AS b", "SELECT 2"]
        );
        assert_eq!(
            split("SELECT \"we;ird\" FROM t; SELECT 2"),
            vec!["SELECT \"we;ird\" FROM t", "SELECT 2"]
        );
        assert_eq!(
            split("SELECT 1 -- a; b\n; SELECT 2"),
            vec!["SELECT 1 -- a; b", "SELECT 2"]
        );
        assert_eq!(
            split("SELECT /* a; b */ 1; SELECT 2"),
            vec!["SELECT /* a; b */ 1", "SELECT 2"]
        );
        // 嵌套块注释。
        assert_eq!(
            split("SELECT /* outer /* inner; */ still; */ 1; SELECT 2").len(),
            2
        );
    }

    #[test]
    fn handles_escape_strings() {
        // E'...' 中的 \' 不结束字符串。
        assert_eq!(
            split("SELECT E'it\\'s; ok' AS x; SELECT 2"),
            vec!["SELECT E'it\\'s; ok' AS x", "SELECT 2"]
        );
    }

    #[test]
    fn does_not_treat_positional_params_as_dollar_quotes() {
        assert_eq!(
            split("SELECT $1, $2 FROM t WHERE x = $3; SELECT 2"),
            vec!["SELECT $1, $2 FROM t WHERE x = $3", "SELECT 2"]
        );
        // `$` 出现在标识符中间（`a$b`）不是 dollar-quote。
        assert_eq!(
            split("SELECT a$b FROM t; SELECT 2"),
            vec!["SELECT a$b FROM t", "SELECT 2"]
        );
    }

    #[test]
    fn rejects_unterminated_lexical_units() {
        for script in [
            "SELECT 'unterminated",
            "SELECT \"unterminated",
            "SELECT 1 /* unterminated",
            "SELECT $$unterminated",
            "SELECT $tag$ unterminated",
        ] {
            assert!(
                matches!(split_sql_script(script), Err(DbError::InvalidArgument(_))),
                "应拒绝未闭合脚本: {script:?}"
            );
        }
    }
}
