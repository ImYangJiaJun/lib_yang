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
    return [target for target in integration_test_targets() if target not in present]


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

    print(f"CI contract verified: {args.workflow}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
