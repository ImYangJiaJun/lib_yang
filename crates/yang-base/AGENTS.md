# yang-base — Backend Services Library

**Parent:** lib_yang workspace

## OVERVIEW
Provides plugin management, database access (MySQL/Redis), HTTP client, JWT tokens, action system, table configuration, and error handling — the application-facing layer built on yang-db.

## STRUCTURE
```
yang-base/
├── src/
│   ├── lib.rs           # 8 public modules exported
│   ├── database/        # GlobalDatabase + GlobalRedis init
│   ├── plugin/          # Plugin trait + PluginManager (TODO: JSON Schema)
│   ├── action/          # Action trait + builtin actions
│   │   └── builtin/     # select, get, insert, update, delete
│   ├── http/            # HttpClient (reqwest wrapper)
│   ├── token/           # JWT TokenManager
│   ├── table/           # Table struct definitions
│   ├── router/          # Router configuration
│   └── error/           # BaseError type
├── tests/               # Integration tests (9 files, Docker required)
├── docs/                # API docs, guides, reference
└── examples/            # Example usage (4 files)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Database init | `src/database/mod.rs` | GlobalDatabase::init(), GlobalRedis::init() |
| Plugin management | `src/plugin/mod.rs` | Plugin trait, init_sql(), migration_sql() |
| Custom actions | `src/action/mod.rs` + `src/action/builtin/` | Implement Action trait |
| HTTP requests | `src/http/mod.rs` | Global singleton HttpClient |
| Token generation/validation | `src/token/mod.rs` | TokenManager with JWT |
| Table configuration | `src/table/mod.rs` | Table struct, field definitions |
| Error handling | `src/error/mod.rs` | BaseError with error codes |
| Full documentation | `docs/api/`, `docs/guides/` | Usage guides, quick reference, Redis guide |

## CONVENTIONS
- Depends on `yang-db` for all database types (DatabaseConfig, RedisConfig, etc.)
- Uses global singletons: `GlobalDatabase`, `GlobalRedis`, `HttpClient`
- All public modules re-exported from `lib.rs`
- Error type: `BaseError` (extends `std::error::Error`)

## ANTI-PATTERNS
- **TODO**: Plugin JSON Schema validation not implemented
- **TODO**: `builtin/select.rs` and `builtin/get.rs` use `serde_json::Value` — needs concrete types
- Test files use excessive `unwrap()` — replace with proper error handling

## 测试约定

### 禁止裸 `.unwrap()`
测试代码中**禁止**使用裸 `.unwrap()`，必须使用 `.expect("<具体上下文>")` 替代，以便在测试失败时提供清晰的错误信息。

```rust
// ❌ 禁止
let result = some_fn().unwrap();

// ✅ 正确
let result = some_fn().expect("调用 some_fn 应该成功");
```

### 禁止 `panic!` 断言错误类型
测试中断言错误类型时，**禁止**使用 `panic!("期望 XXX 错误")`，必须使用 `assert!(matches!(...))` 替代：

```rust
// ❌ 禁止
if let Err(MyError::Foo) = result {
    // 验证逻辑
} else {
    panic!("期望 Foo 错误");
}

// ✅ 正确
assert!(
    matches!(result, Err(MyError::Foo)),
    "期望 Foo 错误，实际: {:?}",
    result
);
```

## 凭证注入流程

### 背景
项目中的数据库连接凭证（如 MySQL 密码）**不得**以明文形式提交到版本控制系统。`.mcp.json` 使用占位符 `${MYSQL_TEST_PASSWORD}`，实际密码通过环境变量或本地配置文件注入。

### 方式一：环境变量注入（推荐）

在运行测试或启动 MCP 服务前，设置以下环境变量：

```bash
# Linux / macOS
export MYSQL_TEST_PASSWORD=your_password_here

# Windows PowerShell
$env:MYSQL_TEST_PASSWORD = "your_password_here"

# Windows CMD
set MYSQL_TEST_PASSWORD=your_password_here
```

测试代码中通过以下方式读取密码：

```rust
let password = std::env::var("MYSQL_TEST_PASSWORD")
    .unwrap_or_else(|_| "111111".to_string());
```

### 方式二：本地配置文件（`.mcp.local.json`）

创建 `.mcp.local.json`（已加入 `.gitignore`，不会提交到版本库），内容与 `.mcp.json` 相同但填入真实密码：

```json
{
  "mcpServers": {
    "mysql_ro": {
      "env": {
        "MYSQL_PASSWORD": "your_actual_password"
      }
    }
  }
}
```

### 注意事项
- `.mcp.json` 中的 `${MYSQL_TEST_PASSWORD}` 是占位符，**不是**实际密码
- `.mcp.local.json` 已加入 `.gitignore`，可安全存放真实密码
- CI/CD 环境通过 Secrets 管理器注入环境变量
- 本地开发默认密码为 `111111`（仅用于本地 Docker 测试容器）
