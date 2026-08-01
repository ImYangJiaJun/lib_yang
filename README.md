# lib_yang

开发、提交与本地 CI 要求见 [CONTRIBUTING.md](CONTRIBUTING.md)。

YANG Rust workspace，包含后端基础库、过程式地图生成库，以及用于联合调试基础库的基础系统应用。

## Workspace 结构

```text
lib_yang/
├── crates/
│   ├── yang-db/            # MySQL/PostgreSQL 查询与 Redis 客户端
│   ├── yang-base/          # 后端服务、Action、路由、Token 与数据库编排
│   ├── yang-base-derive/   # Action 派生宏
│   ├── yang-runtime/       # 配置源、可观测性与进程生命周期
│   └── yang-pcg/           # UE5/Roguelike 过程式地图生成
└── project/
    └── yang-system/        # 基于 yang-base 的基础系统与联合调试入口
```

`project/yang-system` 是独立 Git/Cargo 项目，并从根 workspace 排除。它通过相对路径直接依赖 `../../crates/yang-base`、`../../crates/yang-db` 和 `../../crates/yang-runtime`，因此基础库修改会直接参与基础系统编译，无需 Cargo patch。

`yang-base 0.2.0` 的注册边界同时承担契约和安全校验：同一 `ModuleRouter` 可以组合公开与受保护 API，`TokenAuthMiddleware` 只拦截受保护 Action；`.crud()` 自动为写接口生成 `{module}:write`、为读接口生成 `{module}:read`，并把具体 `TableDefinition` 的字段、主键和查询能力投影到 `ApiCatalog`。路由模板在注册/目录构建期按 Axum 0.8 的 `{name}` / `{*name}` 语法检查，`TableQuery` 则在生成 SQL 前校验 WHERE 字段、操作符和值类型，并把与 `null` 的等值比较规范化为 `IS NULL` / `IS NOT NULL`。

## 本地开发环境

### 必需工具

| 工具 | 版本与用途 |
|---|---|
| Rustup | 安装仓库固定的 Rust 1.97.1、rustfmt 和 clippy |
| Python | 3.11+，运行本地 CI 脚本 |
| Docker Desktop | 提供 Docker Compose、MySQL 8.0 和 Redis 7 |
| Node.js | 24+，运行 `yang-system` 前端 |
| Corepack | 启用项目锁定的 pnpm 10.33.1 |

Docker Desktop 必须先启动。首次配置在仓库根目录执行：

```powershell
pwsh -NoProfile -File project/yang-system/scripts/setup_local.ps1
```

脚本会检查工具链、启用 pnpm 10.33.1、启动 MySQL/Redis、安装前端依赖，并在不存在时生成 `project/yang-system/config.toml`。该配置含随机 Token 密钥且被 Git 忽略；已有配置不会被覆盖。

只检查必需工具而不启动或修改环境：

```powershell
pwsh -NoProfile -File project/yang-system/scripts/setup_local.ps1 -CheckOnly
```

### 编译基础库

根 workspace 包含 `yang-base`、`yang-base-derive`、`yang-db`、`yang-runtime` 和 `yang-pcg`：

```powershell
cargo check --workspace --all-targets --locked
python scripts/run_ci.py quick
```

### 启动基础系统

后端必须从独立项目目录启动，因为它从当前目录读取 `config.toml`：

```powershell
Set-Location project/yang-system
cargo check --all-targets --locked
cargo run --locked
```

另开终端启动前端：

```powershell
Set-Location project/yang-system
pnpm --dir frontend dev
```

| 服务 | 本地地址 |
|---|---|
| 后端 HTTP | `http://127.0.0.1:8080` |
| 存活 / 就绪检查 | `http://127.0.0.1:8080/health/live` / `http://127.0.0.1:8080/health/ready` |
| 前端 | `http://127.0.0.1:5173` |
| MySQL | `127.0.0.1:3306`，数据库 `yang_system` |
| Redis | `127.0.0.1:6379` |

前端已经将 `/api`、`/.well-known` 和 `/health` 代理到本地后端。

### 集成测试

真实依赖集成测试使用独立的 `yang_system_test` 数据库和 Redis DB 15：

```powershell
Set-Location project/yang-system
$env:YANG_SYSTEM_TEST_DATABASE_URL = "mysql://root:yang-local@127.0.0.1:3306/yang_system_test"
$env:YANG_SYSTEM_TEST_REDIS_URL = "redis://127.0.0.1:6379/15"
cargo test --test system_integration -- --ignored --test-threads=1
```

这些环境变量只供集成测试读取；应用运行配置只读取 `config.toml`，不接受环境变量覆盖。

### 停止与重置依赖

```powershell
docker compose -f project/yang-system/compose.yaml down
```

需要彻底重建本地数据时才执行以下命令。它会永久删除 Compose 管理的 MySQL 和 Redis 数据卷：

```powershell
docker compose -f project/yang-system/compose.yaml down -v
```

## 常用命令

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

基础系统的架构、配置项和 HTTP API 参见 [`project/yang-system/README.md`](project/yang-system/README.md)。
