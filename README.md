# lib_yang

YANG Rust workspace，包含后端基础库、过程式地图生成库，以及用于联合调试基础库的基础系统应用。

## Workspace 结构

```text
lib_yang/
├── crates/
│   ├── yang-db/            # MySQL/PostgreSQL 查询与 Redis 客户端
│   ├── yang-base/          # 后端服务、Action、路由、Token 与数据库编排
│   ├── yang-base-derive/   # TableEntity 与 Action 派生宏
│   └── yang-pcg/           # UE5/Roguelike 过程式地图生成
└── project/
    └── yang-system/        # 基于 yang-base 的基础系统与联合调试入口
```

`project/yang-system` 保持为独立 Git/Cargo 项目，并从根 workspace 排除。当前 `yang-base 0.1.3` 尚未发布到 crates.io，因此应用固定依赖 `lib_yang` 的 Git revision；它仍可脱离本仓库单独 clone、构建和运行。

需要联合调试当前基础库源码时，在 `project/yang-system` 目录通过临时 Cargo patch 覆盖依赖，不修改可独立发布的应用清单：

```powershell
cargo --config 'patch."ssh://git@github.com/ImYangJiaJun/lib_yang.git".yang-base.path="../../crates/yang-base"' `
      --config 'patch."ssh://git@github.com/ImYangJiaJun/lib_yang.git".yang-db.path="../../crates/yang-db"' `
      test --all-targets
```

临时 patch 会让应用的 `Cargo.lock` 记录本地源；联调结束后恢复该锁文件，避免把仅本机可用的依赖来源提交进独立仓库。

## 常用命令

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

从仓库根目录启动基础系统时，需要显式指定配置文件；数据库、Redis 与 Token secret 仍通过环境变量注入：

```powershell
$env:DATABASE_URL = "mysql://root:password@127.0.0.1:3306/yang_system"
$env:REDIS_URL = "redis://127.0.0.1:6379"
$env:TOKEN_SECRET = "replace-with-at-least-32-random-bytes"
Set-Location project/yang-system
cargo run
```

基础系统的架构、配置项和 HTTP API 参见 [`project/yang-system/README.md`](project/yang-system/README.md)。
