#!/usr/bin/env python3
"""校验基础库 CI 是否保留计划要求的关键门禁。"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


REQUIRED_FRAGMENTS = (
    "stable:",
    "supply-chain:",
    "msrv:",
    "feature-matrix:",
    "docker-mysql:",
    "docker-postgres:",
    "docker-postgres-17:",
    "docker-redis:",
    'CI_RUST_VERSION: "1.97.1"',
    "toolchain: ${{ env.CI_RUST_VERSION }}",
    "cargo fmt --all -- --check",
    "cargo test --lib -p yang-db --locked",
    "cargo test --lib -p yang-base --locked",
    "cargo test --all-targets -p yang-base-derive -p yang-pcg -p yang-runtime --locked",
    "cargo test --all-targets -p yang-feishu --all-features --locked",
    "cargo clippy -p yang-db -p yang-base -p yang-base-derive -p yang-pcg -p yang-runtime -p yang-feishu --all-targets --all-features --locked -- -D warnings",
    "cargo test --doc -p yang-db --locked",
    "cargo test --doc -p yang-base --locked",
    "cargo test --doc -p yang-runtime --locked",
    "cargo test --doc -p yang-feishu --locked",
    "python scripts/verify_feature_isolation.py --self-test",
    "python scripts/run_ci.py --self-test",
    "python scripts/run_performance_shadow.py --self-test",
    "python scripts/verify_dependency_policy.py --self-test",
    "python scripts/verify_dependency_policy.py",
    "EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25",
    "command: check advisories licenses sources",
    "arguments: --all-features --locked",
    "name: yang-db-none",
    "name: yang-db-mysql",
    "name: yang-db-postgres",
    "name: yang-db-redis",
    "name: yang-db-all",
    "name: yang-base-none",
    "name: yang-base-token",
    "name: yang-base-http",
    "name: yang-base-mysql",
    "name: yang-base-postgres",
    "name: yang-base-mysql-postgres",
    "name: yang-base-redis",
    "name: yang-base-validator",
    "name: yang-base-plugin-schema",
    "name: yang-base-metrics",
    "name: yang-base-openapi",
    "name: yang-base-admin-metadata",
    "name: yang-base-default",
    "name: yang-base-all",
    "name: yang-feishu-none",
    "name: yang-feishu-default",
    "name: yang-feishu-bitable",
    "name: yang-feishu-docs",
    "name: yang-feishu-api-all",
    "name: yang-feishu-http",
    "name: yang-feishu-redis",
    "name: yang-feishu-metrics",
    "name: yang-feishu-all",
    "cargo check -p ${{ matrix.package }} ${{ matrix.args }} --locked",
    "cargo test --lib -p ${{ matrix.package }} ${{ matrix.args }} --locked",
    "cargo test --doc -p ${{ matrix.package }} ${{ matrix.args }} --locked",
    "cargo check --workspace --all-targets --all-features --locked",
    'toolchain: "1.80"',
    "REDIS_TEST_URL: redis://127.0.0.1:6379",
    "--health-cmd \"redis-cli ping\"",
    "cargo test -p yang-db --test integration_redis_pipeline --locked -- --include-ignored --test-threads=1",
    "cargo test -p yang-db --test integration_redis_script --locked -- --include-ignored --test-threads=1",
    "cargo test -p yang-db --test integration_redis_transaction --locked -- --include-ignored --test-threads=1",
    "cargo test -p yang-db --test integration_redis_string_list_ops --locked -- --include-ignored --test-threads=1",
    "cargo test -p yang-db --test integration_redis_mature --locked -- --include-ignored --test-threads=1",
    "for suite in test_redis_client test_redis_config test_redis_list_ops test_redis_pipeline test_redis_script test_redis_string_ops; do",
    "cargo test -p yang-base --test cache_integration --locked -- --test-threads=1",
    "cargo test -p yang-base --test token_revocation_integration --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --test email_verification_integration --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_record_pg --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_schema_sync_pg --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_table_query_pg --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_table_query_pg_phase_c --locked -- --ignored --test-threads=1",
    'PG_TEST_URL: postgres://postgres:postgres@localhost:5432/test',
    "performance-shadow:",
    "runs-on: ubuntu-24.04",
    "continue-on-error: true",
    "python scripts/run_performance_shadow.py --output \"${{ runner.temp }}/performance-shadow\"",
    "actions/upload-artifact@v7",
    "if: always()",
    "path: ${{ runner.temp }}/performance-shadow",
    "if-no-files-found: error",
    "mysql:8.0",
    "postgres:16-alpine",
    "postgres:17-alpine",
    "redis:7-alpine",
    "cargo test -p yang-db --test integration_pg_phase_a --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --test integration_pg_phase_b_queries --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --test integration_pg_phase_b_runtime --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --test integration_pg_phase_b_types --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --test integration_pg_phase_b_copy_listen --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --no-default-features --features postgres --test integration_pg_phase_c --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --no-default-features --features postgres --test proptest_pg_types --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --no-default-features --features postgres --test integration_pg_p0p1_wave1 --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_schema_sync_pg_wave1 --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --no-default-features --features postgres --test integration_pg_wave2_query --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --no-default-features --features postgres --test integration_pg_wave2_errors --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_pg_wave2_read --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_pg_wave2_write --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_pg_wave2_addaction --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --no-default-features --features postgres --test integration_pg_wave3_contract --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --no-default-features --features postgres --test integration_pg_wave3_query_features --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_pg_wave3_guards --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_pg_wave3_runtime --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_schema_sync_pg_wave3 --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_pg_wave4_types --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_pg_wave4_codec --locked -- --ignored --test-threads=1",
    "--test-threads=1",
)


def missing_contract_fragments(text: str) -> list[str]:
    """返回 CI 文本中缺失的必需契约片段。"""

    return [fragment for fragment in REQUIRED_FRAGMENTS if fragment not in text]


def integration_test_targets() -> tuple[str, ...]:
    """从本地 runner 的 integration 命令提取 --test 目标。"""

    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import run_ci

    targets = []
    for command in run_ci.INTEGRATION_COMMANDS:
        argv = command.argv
        for index, argument in enumerate(argv[:-1]):
            if argument == "--test":
                targets.append(argv[index + 1])
    return tuple(dict.fromkeys(targets))


def missing_integration_targets(text: str) -> list[str]:
    """返回 workflow 文本中缺失的本地 integration 目标。"""

    present = set(re.findall(r"--test\s+(\S+)", text))
    # Redis 命令套件在 workflow 中用 `for suite in ...; do` 循环执行。
    for group in re.findall(r"for suite in ([^;]+);\s*do", text):
        present.update(token.strip("\"'") for token in group.split())
    return [target for target in integration_test_targets() if target not in present]


REDIS_TEST_FILES = (
    "crates/yang-base/tests/token_revocation_integration.rs",
    "crates/yang-base/tests/email_verification_integration.rs",
    "crates/yang-base/tests/cache_integration.rs",
)

REDIS_CONNECT_MARKERS = (
    "RedisClient::connect(",
    "RedisClient::connect_with_config(",
    "RedisClient::connect_topology(",
)

SOFT_SKIP_PATTERNS = (
    re.compile(r"is_err\(\)\s*\{\s*return\s*;", re.S),
    re.compile(r"is_err\(\)\s*\{[^{}]*return\s*;", re.S),
    re.compile(r"跳过测试|跳过集成测试"),
)


def text_soft_skip_violations(text: str, name: str) -> list[str]:
    """检查单个测试文件文本是否存在空壳/软跳过。"""

    violations: list[str] = []
    if "REDIS_TEST_URL" not in text:
        violations.append(f"{name}: 未使用 REDIS_TEST_URL（无法复用测试专用实例）")
    if not any(marker in text for marker in REDIS_CONNECT_MARKERS):
        violations.append(f"{name}: 未发现真实 RedisClient 连接调用（可能是空壳测试）")
    for pattern in SOFT_SKIP_PATTERNS:
        if pattern.search(text):
            violations.append(f"{name}: 仍包含失败即跳过（soft-skip）模式")
            break
    return violations


def soft_skip_violations(root: Path) -> list[str]:
    """检查 Redis 集成测试是否仍存在「失败即跳过」的绿色假象。"""

    violations: list[str] = []
    # 拓扑测试使用 REDIS_TLS_URL / REDIS_CLUSTER_URLS / REDIS_SENTINEL_URLS，
    # 需要外部启动 TLS/Cluster/Sentinel，不属于 REDIS_TEST_URL 标准套件。
    candidates = [
        path
        for path in root.glob("crates/yang-db/tests/*redis*.rs")
        if "topologies" not in path.name
    ]
    candidates.extend(root / path for path in REDIS_TEST_FILES)

    for path in candidates:
        if not path.exists():
            violations.append(f"{path}: Redis 集成测试文件缺失")
            continue
        text = path.read_text(encoding="utf-8")
        violations.extend(text_soft_skip_violations(text, str(path)))
    return violations


def run_self_test() -> None:
    """用恶意删减 fixture 证明校验器能拒绝缺失门禁。"""

    valid_fixture = "\n".join(REQUIRED_FRAGMENTS)
    assert not missing_contract_fragments(valid_fixture)

    for fragment in REQUIRED_FRAGMENTS:
        adversarial_fixture = valid_fixture.replace(fragment, "")
        missing = missing_contract_fragments(adversarial_fixture)
        assert fragment in missing, f"未检测到被删除的 CI 契约: {fragment}"

    targets = integration_test_targets()
    target_fixture = "\n".join(f"--test {target}" for target in targets)
    assert not missing_integration_targets(target_fixture)
    for target in targets:
        adversarial_fixture = target_fixture.replace(f"--test {target}", "", 1)
        missing = missing_integration_targets(adversarial_fixture)
        assert target in missing, f"未检测到被删除的 integration 目标: {target}"

    # 软跳过检测器本身必须能识别真实的失败即 return 模式
    malicious = """
    fn redis_test_url() -> String { String::new() }
    async fn bad() {
        if RedisClient::connect(redis_test_url()).await.is_err() { return; }
    }
    """
    assert text_soft_skip_violations(malicious, "<malicious>"), (
        "soft-skip 检测器未能识别失败即 return"
    )

    clean = """
    fn redis_test_url() -> String { std::env::var("REDIS_TEST_URL").unwrap() }
    async fn good() {
        let client = RedisClient::connect(redis_test_url()).await.unwrap();
        let _ = client;
    }
    """
    assert not text_soft_skip_violations(clean, "<clean>"), (
        "soft-skip 检测器误报了正常测试"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("workflow", nargs="?", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        run_self_test()

    if args.workflow is None:
        if args.self_test:
            print("CI contract self-test passed")
            return 0
        parser.error("必须提供 workflow 路径或 --self-test")

    text = args.workflow.read_text(encoding="utf-8")
    missing = missing_contract_fragments(text)
    missing_targets = missing_integration_targets(text)
    if missing or missing_targets:
        for fragment in missing:
            print(f"missing CI contract fragment: {fragment}")
        for target in missing_targets:
            print(f"missing local integration target in CI: {target}")
        return 1

    violations = soft_skip_violations(Path.cwd())
    if violations:
        for violation in violations:
            print(f"soft-skip violation: {violation}")
        return 1

    print(f"CI contract verified: {args.workflow}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
