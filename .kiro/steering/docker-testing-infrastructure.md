---
inclusion: auto
---

# Docker 测试基础设施规则

## 环境检查与创建流程（最高优先级）

执行测试前按以下顺序操作：

### 步骤 1：检查目标服务是否已存在且可用

| 服务 | 检查方式 | 判定可用 |
|------|---------|---------|
| MySQL 容器 | `docker exec mysql-test mysqladmin ping -u root -p111111 --silent 2>&1` | 退出码 0 |
| Redis 容器 | `docker exec redis-test redis-cli ping 2>&1` | 返回 "PONG" |
| MySQL 本地 | `mysql -u root -p111111 -h 127.0.0.1 -e "SELECT 1" 2>&1` | 退出码 0 |
| Redis 本地 | `redis-cli -h 127.0.0.1 ping 2>&1` | 返回 "PONG" |

### 步骤 2：不存在或不可用时自动创建

如果容器不存在或已停止：

```bash
# 检查容器状态
docker ps -a --filter name=mysql-test
docker ps -a --filter name=redis-test

# 不存在 → 创建
docker run -d --name mysql-test -p 3306:3306 -e MYSQL_ROOT_PASSWORD=111111 -e MYSQL_DATABASE=test mysql:8.0
docker run -d --name redis-test -p 6379:6379 redis:7-alpine

# 存在但停止 → 启动
docker start mysql-test
docker start redis-test
```

### 步骤 3：等待就绪

```powershell
# MySQL（最多 60 秒）
for ($i=0; $i -lt 30; $i++) {
  try { docker exec mysql-test mysqladmin ping -u root -p111111 --silent 2>&1 | Out-Null; if ($LASTEXITCODE -eq 0) { break } } catch {}
  Start-Sleep -Seconds 2
}

# Redis（最多 10 秒）
for ($i=0; $i -lt 10; $i++) {
  try { $r = docker exec redis-test redis-cli ping 2>&1; if ($r -eq "PONG") { break } } catch {}
  Start-Sleep -Seconds 1
}
```

### 步骤 4：运行测试

```bash
cargo test
```

### 步骤 5：环境判断与清理

- **测试中创建的容器**：必须清理（`docker rm -f 容器名`）
- **预先存在的容器**：保留不动
- 判断标准：记录容器创建前是否已存在

## 不可用场景识别

| 场景 | 表现 | 处理 |
|------|------|------|
| 容器不存在 | `docker ps -a` 无记录 | 创建新容器 |
| 容器已停止 | `docker ps` 无 running 记录 | `docker start` 或重建 |
| 密码错误 | 认证失败 | 创建新容器（标准密码） |
| 端口冲突 | 端口被占用 | `docker ps` 确认占用人，换端口或停用 |
| 本地服务不可达 | 连接拒绝 | 改用 Docker 容器 |
| MySQL 数据库缺失 | 连接成功但库不存在 | `docker exec mysql-test mysql -u root -p111111 -e "CREATE DATABASE IF NOT EXISTS test"` |

## 服务标准配置

| 服务 | 镜像 | 容器名 | 端口 | 凭据 |
|------|------|--------|------|------|
| MySQL | `mysql:8.0` | `mysql-test` | 3306:3306 | root / 111111 / test |
| Redis | `redis:7-alpine` | `redis-test` | 6379:6379 | 无密码 |

## 核心原则

1. **先检查，再决定**：不预设环境存在或不存在
2. **能连就用**：已有可用服务直接使用，不重复创建
3. **不通就建**：不可用（容器缺失/停止/密码错误/无权限）一律创建标准容器
4. **建了要清**：测试中新建的容器测试后必须清理
5. **原有的留**：测试前已存在的服务保持原样

---

**创建日期**: 2026-05-02
**最后更新**: 2026-05-03
**适用范围**: 全局（yang-db 项目及本机所有项目）
