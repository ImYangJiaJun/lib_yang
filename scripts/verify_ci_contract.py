#!/usr/bin/env python3
"""校验基础库 CI 是否保留计划要求的关键门禁。"""

from __future__ import annotations

import argparse
from pathlib import Path


REQUIRED_FRAGMENTS = (
    "stable:",
    "msrv:",
    "feature-matrix:",
    "docker-mysql:",
    "docker-postgres:",
    "docker-redis:",
    "cargo fmt --all -- --check",
    "cargo test --lib -p yang-db --locked",
    "cargo test --lib -p yang-base --locked",
    "cargo clippy -p yang-db -p yang-base --all-targets --all-features --locked -- -D warnings",
    "cargo test --doc -p yang-db --locked",
    "cargo test --doc -p yang-base --locked",
    "cargo check -p yang-base --no-default-features --locked",
    "cargo check -p yang-base --all-features --locked",
    "cargo check -p yang-db -p yang-base --all-features --locked",
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
        adversarial_fixture = valid_fixture.replace(fragment, "", 1)
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
