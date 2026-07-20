# 本地开发环境设计

## 目标

为 Windows 开发机提供可重复、低摩擦的本地环境，使开发者能够：

- 编译根 workspace 中全部 `yang-*` crate；
- 编译、启动并验证 `project/yang-system` 后端；
- 安装、构建并启动 `project/yang-system/frontend`；
- 使用与 CI 主版本一致的 MySQL 8、Redis 7 运行系统和真实依赖集成测试；
- 从 README 准确获知所有必需工具、配置、命令和端口。

## 方案

采用“宿主机编译 + Docker 依赖服务”模式。Rust、Node.js 与 pnpm 直接运行在
Windows 上，保留本地增量编译和源码联调体验；MySQL 与 Redis 由 Docker Compose
统一管理，避免要求开发者安装和维护本机数据库服务。

`yang-system` 仍通过相对路径依赖 `../../crates/yang-base` 与
`../../crates/yang-db`，不引入 Cargo patch、Git revision 或绝对路径。

## 组件与文件

### Docker 依赖

在 `project/yang-system/compose.yaml` 定义 MySQL 8 与 Redis 7：

- 端口固定为 `3306` 与 `6379`；
- 使用命名卷保存本地开发数据；
- 配置健康检查，初始化脚本等待服务真正可用；
- MySQL 初始化 `yang_system` 和 `yang_system_test` 两个数据库；
- 本地示例凭据仅用于 Docker 开发环境，不用于生产部署。

MySQL 初始化 SQL 独立存放在
`project/yang-system/docker/mysql/init/001-create-databases.sql`。

### 本地初始化脚本

新增 `project/yang-system/scripts/setup_local.ps1`，并保持幂等：

1. 检查 `rustup`、Cargo、Docker、Node.js 与 Corepack；
2. 确认仓库固定的 Rust 工具链及 `rustfmt`、`clippy` 可用；
3. 通过 Corepack 启用 `package.json` 固定的 pnpm 10.33.1；
4. 启动 Compose 服务并等待健康状态；
5. 仅在文件不存在时，从示例生成被 Git 忽略的 `config.toml`，写入本地连接地址和随机 Token 密钥；
6. 使用冻结锁文件安装前端依赖。

脚本不覆盖已有 `config.toml`，不删除数据卷，也不修改用户级数据库配置。

### 文档

根 `README.md` 增加完整的本地开发入口，并修正与当前代码不一致的内容：

- 列出 Rustup、Python 3.11+、Docker Desktop、Node.js 24 与 Corepack；
- 说明根 workspace 和 `yang-system` 是两个独立 Cargo/Git 项目；
- 删除旧的 Git revision、临时 Cargo patch 和环境变量启动说明；
- 给出初始化、编译、启动、前后端联调、集成测试、停止服务和重置数据命令；
- 明确服务端口及 `config.toml` 的安全边界。

`project/yang-system/README.md` 同步记录 Compose 和初始化脚本的细节，保留现有架构说明。

## 配置与安全

- `config.toml` 继续由嵌套项目的 `.gitignore` 排除，应用运行时只读取该文件；
- Token 密钥使用加密安全随机数生成，长度不少于 32 字节；
- 开发数据库密码仅出现在 Compose 示例、本地生成配置和 README 开发示例中；
- 集成测试继续使用 `yang_system_test` 和 Redis DB 15，避免污染开发数据；
- 数据卷重置是显式破坏性操作，只在 README 中提供单独命令，不由初始化脚本自动执行。

## 验证

实施后执行以下验证：

1. 根目录 `cargo check --workspace --all-targets --locked`；
2. 根目录 `python scripts/run_ci.py quick`；
3. `project/yang-system` 下 `cargo check --all-targets --locked`；
4. `project/yang-system` 下 `python scripts/run_ci.py quick`；
5. 使用 Docker 依赖运行被忽略的真实集成测试；
6. 启动后端并请求 `/health/ready`；
7. 前端执行 `pnpm check`，再启动开发服务器并验证首页可访问。

如果完整门禁暴露与本次环境配置无关的既有失败，保留失败证据并将其与环境配置结果分开报告。
