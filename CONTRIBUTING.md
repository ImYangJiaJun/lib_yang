# 贡献与提交规范

本仓库以 `.github/workflows/ci.yml` 为最终质量契约。所有代码、依赖、文档示例和
feature 调整都必须在提交前通过对应的本地门禁。

## 提交流程

1. 保持改动聚焦，不提交本地 IDE 配置、临时目录或无关文件。
2. 开发过程中运行受影响 crate 的最小测试。
3. 提交前运行快速门禁：

   ```bash
   python scripts/run_ci.py quick
   ```

4. 推送前运行完整非 Docker 门禁：

   ```bash
   python scripts/run_ci.py full
   ```

5. 涉及 MySQL、PostgreSQL 或 Redis 行为时，在对应服务可用后运行：

   ```bash
   python scripts/run_ci.py integration
   ```

`integration` 需要与 CI 相同的 MySQL 8.0、PostgreSQL 16 和 Redis 7 环境；Docker
集成测试必须保持单线程执行。

## 强制约束

- `Cargo.lock` 必须提交，所有门禁使用 `--locked`。
- 默认开发与 CI toolchain 固定在 `rust-toolchain.toml`；升级时必须同步 CI 契约并完整验证，
  避免不同 rustfmt 版本产生互相冲突的格式。
- 项目 MSRV 为 Rust 1.80；任何依赖更新都必须重新运行完整门禁，不得用提高 MSRV
  掩盖依赖漂移。
- 每个 Cargo feature 必须能独立通过 `check`、library tests 和 doctests，且警告按错误处理。
- 公共 API 变更必须同步更新可编译 doctest；示例不得继续使用已废弃的参数类型。
- feature 专用实现必须用准确的 `cfg` 边界，不能用宽泛 `allow(dead_code)` 隐藏隔离问题。
- 权限测试必须显式声明 `searchable`、`filterable`、`sortable` 等能力，保持 fail-closed。
- 不删除、忽略或弱化失败测试来绕过门禁。

## CI 维护

修改 `.github/workflows/ci.yml` 时必须同步更新 `scripts/verify_ci_contract.py` 和
`scripts/run_ci.py`。CI 会执行两个脚本的 self-test，防止本地规范与远端门禁静默漂移。
