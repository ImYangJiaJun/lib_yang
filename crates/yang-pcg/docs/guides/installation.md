# 构建与发布

## 构建库

```bash
cargo build --release -p yang-pcg
```

## 构建命令行工具 pcg_cli

`pcg_cli` 用于运行时生成地图（UE5 集成路线 B）。

```bash
cargo build --release --bin pcg_cli -p yang-pcg
# 产物：target/release/pcg_cli(.exe)
```

用法：

```bash
pcg_cli --seed 12345 --out floor.json              # JSON
pcg_cli --seed 12345 --format binary --out floor.ypcg   # 二进制 + CRC32
pcg_cli --config dungeon.json --out floor.json     # 从 JSON 文件加载配置
pcg_cli --help
```

退出码：`0` 成功 / `1` 参数错误 / `2` 配置读取失败 / `3` 生成失败 / `4` 写入失败。

完整说明见 [UE5_INTEGRATION.md](../../UE5_INTEGRATION.md) 第 5 节。

## 发布

```bash
cargo publish -p yang-pcg
```

## 代码检查

```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib -p yang-pcg
```
