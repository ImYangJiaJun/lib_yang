#!/usr/bin/env python3
"""校验基础库 CI 是否保留计划要求的关键门禁。"""

from __future__ import annotations

import argparse
from pathlib import Path


REQUIRED_FRAGMENTS = (
    "stable:",
    "supply-chain:",
    "msrv:",
    "feature-matrix:",
    "docker-mysql:",
    "docker-postgres:",
    "docker-redis:",
    'CI_RUST_VERSION: "1.97.1"',
    "toolchain: ${{ env.CI_RUST_VERSION }}",
    "cargo fmt --all -- --check",
    "cargo test --lib -p yang-db --locked",
    "cargo test --lib -p yang-base --locked",
    "cargo test --all-targets -p yang-base-derive -p yang-migrate -p yang-pcg --locked",
    "cargo clippy -p yang-db -p yang-base -p yang-base-derive -p yang-migrate -p yang-pcg --all-targets --all-features --locked -- -D warnings",
    "cargo test --doc -p yang-db --locked",
    "cargo test --doc -p yang-base --locked",
    "python scripts/verify_feature_isolation.py --self-test",
    "python scripts/run_ci.py --self-test",
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
    "name: yang-base-redis",
    "name: yang-base-validator",
    "name: yang-base-plugin-schema",
    "name: yang-base-metrics",
    "name: yang-base-openapi",
    "name: yang-base-admin-metadata",
    "name: yang-base-default",
    "name: yang-base-all",
    "cargo check -p ${{ matrix.package }} ${{ matrix.args }} --locked",
    "cargo test --lib -p ${{ matrix.package }} ${{ matrix.args }} --locked",
    "cargo test --doc -p ${{ matrix.package }} ${{ matrix.args }} --locked",
    "cargo check --workspace --all-targets --all-features --locked",
    'toolchain: "1.80"',
    "mysql:8.0",
    "postgres:16-alpine",
    "redis:7-alpine",
    "--test-threads=1",
)


def missing_contract_fragments(text: str) -> list[str]:
    """返回 CI 文本中缺失的必需契约片段。"""

    return [fragment for fragment in REQUIRED_FRAGMENTS if fragment not in text]


def run_self_test() -> None:
    """用恶意删减 fixture 证明校验器能拒绝缺失门禁。"""

    valid_fixture = "\n".join(REQUIRED_FRAGMENTS)
    assert not missing_contract_fragments(valid_fixture)

    for fragment in REQUIRED_FRAGMENTS:
        adversarial_fixture = valid_fixture.replace(fragment, "")
        missing = missing_contract_fragments(adversarial_fixture)
        assert fragment in missing, f"未检测到被删除的 CI 契约: {fragment}"


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
    if missing:
        for fragment in missing:
            print(f"missing CI contract fragment: {fragment}")
        return 1

    print(f"CI contract verified: {args.workflow}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
