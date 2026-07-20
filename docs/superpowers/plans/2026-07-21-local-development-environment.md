# Local Development Environment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 Windows 开发者能够一次初始化并编译、运行全部 `yang-*` 基础库、`yang-system` 后端和 Quasar 前端。

**Architecture:** Rust 与 Node.js 工具链运行在宿主机，MySQL 8 和 Redis 7 由 `project/yang-system/compose.yaml` 管理。幂等 PowerShell 脚本负责工具检查、服务启动、本机配置生成和前端依赖安装；根 README 与嵌套项目 README 共同记录可复现流程。

**Tech Stack:** Rust 1.97.1、PowerShell 7、Docker Compose、MySQL 8.0、Redis 7、Node.js 24、Corepack、pnpm 10.33.1、Quasar/Vite

---

## 文件结构

- Create: `project/yang-system/compose.yaml` — 本地 MySQL/Redis 服务、卷与健康检查。
- Create: `project/yang-system/docker/mysql/init/001-create-databases.sql` — 创建开发及集成测试数据库。
- Create: `project/yang-system/scripts/setup_local.ps1` — 幂等初始化入口。
- Modify: `README.md` — 根 workspace 的统一本地环境入口并删除过时说明。
- Modify: `project/yang-system/README.md` — 后端、前端和依赖服务的详细本地流程。
- Local ignored: `project/yang-system/config.toml` — 当前机器运行配置，不提交。

### Task 1: Docker 依赖服务

**Files:**
- Create: `project/yang-system/compose.yaml`
- Create: `project/yang-system/docker/mysql/init/001-create-databases.sql`

- [ ] **Step 1: 运行缺失配置检查**

Run:

```powershell
docker compose -f project/yang-system/compose.yaml config
```

Expected: FAIL，因为 `compose.yaml` 尚不存在。

- [ ] **Step 2: 写入 Compose 配置**

```yaml
name: yang-system-local

services:
  mysql:
    image: mysql:8.0
    environment:
      MYSQL_ROOT_PASSWORD: yang-local
      MYSQL_DATABASE: yang_system
    ports:
      - "127.0.0.1:3306:3306"
    volumes:
      - mysql-data:/var/lib/mysql
      - ./docker/mysql/init:/docker-entrypoint-initdb.d:ro
    healthcheck:
      test: ["CMD-SHELL", "mysqladmin ping -h 127.0.0.1 -uroot -p$$MYSQL_ROOT_PASSWORD --silent"]
      interval: 5s
      timeout: 5s
      retries: 30
      start_period: 20s
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    ports:
      - "127.0.0.1:6379:6379"
    volumes:
      - redis-data:/data
    command: ["redis-server", "--appendonly", "yes"]
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 20
    restart: unless-stopped

volumes:
  mysql-data:
  redis-data:
```

- [ ] **Step 3: 写入数据库初始化 SQL**

```sql
CREATE DATABASE IF NOT EXISTS yang_system
    CHARACTER SET utf8mb4
    COLLATE utf8mb4_unicode_ci;

CREATE DATABASE IF NOT EXISTS yang_system_test
    CHARACTER SET utf8mb4
    COLLATE utf8mb4_unicode_ci;
```

- [ ] **Step 4: 验证 Compose 渲染结果**

Run:

```powershell
docker compose -f project/yang-system/compose.yaml config --quiet
docker compose -f project/yang-system/compose.yaml config --images
```

Expected: 两条命令退出码为 0，镜像包含 `mysql:8.0` 和 `redis:7-alpine`。

### Task 2: 幂等本地初始化脚本

**Files:**
- Create: `project/yang-system/scripts/setup_local.ps1`

- [ ] **Step 1: 运行缺失脚本检查**

Run:

```powershell
pwsh -NoProfile -File project/yang-system/scripts/setup_local.ps1 -CheckOnly
```

Expected: FAIL，因为脚本尚不存在。

- [ ] **Step 2: 实现初始化脚本**

脚本必须提供 `-CheckOnly` 与 `-SkipFrontendInstall` 开关，并完成以下行为：

```powershell
[CmdletBinding()]
param(
    [switch]$CheckOnly,
    [switch]$SkipFrontendInstall
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$configPath = Join-Path $projectRoot "config.toml"
$configExamplePath = Join-Path $projectRoot "config.example.toml"
$composePath = Join-Path $projectRoot "compose.yaml"

function Assert-Command {
    param([Parameter(Mandatory)][string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "缺少必需命令: $Name"
    }
}

foreach ($command in @("rustup", "cargo", "docker", "node", "corepack")) {
    Assert-Command $command
}

$nodeMajor = [int]((node --version).TrimStart("v").Split(".")[0])
if ($nodeMajor -lt 24) {
    throw "Node.js 版本必须为 24 或更高，当前版本: $(node --version)"
}

docker info *> $null
docker compose version *> $null
rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy

if ($CheckOnly) {
    Write-Host "本地环境必需工具检查通过。"
    exit 0
}

Push-Location $projectRoot
try {
    corepack install --global pnpm@10.33.1
    docker compose -f $composePath up -d --wait

    if (-not (Test-Path -LiteralPath $configPath)) {
        $tokenBytes = [byte[]]::new(48)
        [Security.Cryptography.RandomNumberGenerator]::Fill($tokenBytes)
        $tokenSecret = [Convert]::ToBase64String($tokenBytes)
        $config = Get-Content -Raw -LiteralPath $configExamplePath
        $config = $config.Replace(
            "mysql://root:password@127.0.0.1:3306/yang_system",
            "mysql://root:yang-local@127.0.0.1:3306/yang_system"
        )
        $config = $config.Replace(
            "replace-with-at-least-32-random-bytes",
            $tokenSecret
        )
        Set-Content -LiteralPath $configPath -Value $config -Encoding utf8NoBOM
        Write-Host "已生成本机 config.toml。"
    } else {
        Write-Host "保留已有 config.toml。"
    }

    if (-not $SkipFrontendInstall) {
        pnpm --dir frontend install --frozen-lockfile
    }
} finally {
    Pop-Location
}

Write-Host "本地环境初始化完成。"
Write-Host "后端: cargo run --manifest-path project/yang-system/Cargo.toml"
Write-Host "前端: pnpm --dir project/yang-system/frontend dev"
```

- [ ] **Step 3: 验证只检查模式**

Run:

```powershell
pwsh -NoProfile -File project/yang-system/scripts/setup_local.ps1 -CheckOnly
```

Expected: 输出“本地环境必需工具检查通过”，退出码为 0，不创建 `config.toml`。

### Task 3: README 本地环境契约

**Files:**
- Modify: `README.md`
- Modify: `project/yang-system/README.md`

- [ ] **Step 1: 证明根 README 仍包含过时说明**

Run:

```powershell
rg -n "Git revision|临时 Cargo patch|DATABASE_URL|REDIS_URL|TOKEN_SECRET" README.md
```

Expected: 命中旧的依赖与环境变量说明。

- [ ] **Step 2: 重写根 README 本地开发章节**

文档必须包含：

```markdown
## 本地开发环境

### 必需工具

| 工具 | 版本/用途 |
|---|---|
| Rustup | 自动安装仓库固定的 Rust 1.97.1、rustfmt、clippy |
| Python | 3.11+，运行本地 CI 脚本 |
| Docker Desktop | 提供 Docker Compose、MySQL 8 与 Redis 7 |
| Node.js | 24+，运行基础系统前端 |
| Corepack | 启用项目锁定的 pnpm 10.33.1 |

### 一次初始化

```powershell
pwsh -NoProfile -File project/yang-system/scripts/setup_local.ps1
```
```

同时写明根 workspace 编译命令、后端/前端启动命令、端口、集成测试环境变量、服务停止与显式卷重置命令。

- [ ] **Step 3: 更新嵌套项目 README**

将现有手工复制配置步骤升级为脚本优先流程，同时保留手工配置作为排障入口；明确：

- `docker compose up -d --wait` / `docker compose down`；
- `config.toml` 不接受环境变量覆盖；
- 开发后端 `127.0.0.1:8080`、前端 `127.0.0.1:5173`；
- `docker compose down -v` 会删除本地数据库和 Redis 数据；
- 集成测试变量仍只用于测试进程。

- [ ] **Step 4: 验证文档契约**

Run:

```powershell
if (rg -n "Git revision|临时 Cargo patch|\$env:DATABASE_URL|\$env:REDIS_URL|\$env:TOKEN_SECRET" README.md) { exit 1 }
rg -n "setup_local.ps1|Node.js|pnpm 10.33.1|docker compose down -v|cargo check --workspace" README.md project/yang-system/README.md
```

Expected: 第一条退出码为 0；第二条在两个 README 中命中必需说明。

### Task 4: 配置当前机器

**Files:**
- Local ignored: `project/yang-system/config.toml`
- Generated: `project/yang-system/frontend/node_modules/`

- [ ] **Step 1: 运行初始化脚本**

Run:

```powershell
pwsh -NoProfile -File project/yang-system/scripts/setup_local.ps1
```

Expected: MySQL 与 Redis 健康，`config.toml` 生成，pnpm 10.33.1 依赖安装完成。

- [ ] **Step 2: 验证服务、配置与敏感文件边界**

Run:

```powershell
docker compose -f project/yang-system/compose.yaml ps
git -C project/yang-system check-ignore config.toml frontend/node_modules
corepack pnpm --version
```

Expected: 两个服务为 healthy；忽略检查命中本地文件；pnpm 输出 `10.33.1`。

### Task 5: 编译、测试与运行验证

**Files:**
- No source changes expected

- [ ] **Step 1: 验证全部基础库**

Run:

```powershell
cargo check --workspace --all-targets --locked
python scripts/run_ci.py quick
```

Expected: 两条命令退出码为 0。

- [ ] **Step 2: 验证基础系统后端与前端**

Run from `project/yang-system`:

```powershell
cargo check --all-targets --locked
python scripts/run_ci.py quick
pnpm --dir frontend check
```

Expected: 三条命令退出码为 0。

- [ ] **Step 3: 验证真实依赖集成测试**

Run from `project/yang-system`:

```powershell
$env:YANG_SYSTEM_TEST_DATABASE_URL = "mysql://root:yang-local@127.0.0.1:3306/yang_system_test"
$env:YANG_SYSTEM_TEST_REDIS_URL = "redis://127.0.0.1:6379/15"
cargo test --test system_integration -- --ignored --test-threads=1
```

Expected: 集成测试退出码为 0。

- [ ] **Step 4: 启动后端并验证健康检查**

在隐藏后台进程中从 `project/yang-system` 启动 `cargo run --locked`，轮询
`http://127.0.0.1:8080/health/ready`，期望 HTTP 200；验证后终止本次启动的进程。

- [ ] **Step 5: 启动前端并验证页面**

在隐藏后台进程中从 `project/yang-system/frontend` 启动 `pnpm dev`，轮询
`http://127.0.0.1:5173/`，期望 HTTP 200；验证后终止本次启动的进程。

- [ ] **Step 6: 检查最终差异**

Run:

```powershell
git diff --check
git status --short
git -C project/yang-system diff --check
git -C project/yang-system status --short
```

Expected: 无空白错误；仅出现设计内文件和被忽略的本机产物。
