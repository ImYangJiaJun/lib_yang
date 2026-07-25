#!/usr/bin/env python3
"""验证 Cargo feature 不会静默启用无关数据库驱动。"""

from __future__ import annotations

import argparse
import re
import subprocess
from dataclasses import dataclass


@dataclass(frozen=True)
class Case:
    """单个 normal dependency feature 隔离契约。"""

    name: str
    package: str
    args: tuple[str, ...]
    required: frozenset[str]
    forbidden: frozenset[str]


CASES = (
    Case("yang-db-none", "yang-db", ("--no-default-features",), frozenset(), frozenset({"sqlx", "redis", "deadpool-redis"})),
    Case("yang-db-mysql", "yang-db", ("--no-default-features", "--features", "mysql"), frozenset({"sqlx", "sqlx-mysql"}), frozenset({"sqlx-postgres", "redis", "deadpool-redis"})),
    Case("yang-db-postgres", "yang-db", ("--no-default-features", "--features", "postgres"), frozenset({"sqlx", "sqlx-postgres"}), frozenset({"sqlx-mysql", "redis", "deadpool-redis"})),
    Case("yang-db-redis", "yang-db", ("--no-default-features", "--features", "redis"), frozenset({"redis", "deadpool-redis"}), frozenset({"sqlx", "sqlx-mysql", "sqlx-postgres"})),
    Case("yang-base-none", "yang-base", ("--no-default-features",), frozenset(), frozenset({"sqlx", "redis", "deadpool-redis", "reqwest", "jsonwebtoken", "axum", "tower-http"})),
    Case("yang-base-token", "yang-base", ("--no-default-features", "--features", "token"), frozenset({"jsonwebtoken", "redis", "deadpool-redis"}), frozenset({"sqlx", "sqlx-mysql", "sqlx-postgres", "reqwest", "axum", "tower-http"})),
    Case("yang-base-http", "yang-base", ("--no-default-features", "--features", "http"), frozenset({"reqwest", "tower-http"}), frozenset({"sqlx", "redis", "deadpool-redis", "jsonwebtoken", "axum"})),
    Case("yang-base-mysql", "yang-base", ("--no-default-features", "--features", "mysql"), frozenset({"sqlx", "sqlx-mysql"}), frozenset({"sqlx-postgres", "redis", "deadpool-redis", "reqwest", "jsonwebtoken", "axum", "tower-http"})),
    Case("yang-base-redis", "yang-base", ("--no-default-features", "--features", "redis"), frozenset({"redis", "deadpool-redis"}), frozenset({"sqlx", "sqlx-mysql", "sqlx-postgres", "reqwest", "jsonwebtoken", "axum", "tower-http"})),
    Case("yang-base-transport-axum", "yang-base", ("--no-default-features", "--features", "transport-axum"), frozenset({"axum", "tower-http"}), frozenset({"sqlx", "sqlx-mysql", "sqlx-postgres", "redis", "deadpool-redis", "reqwest", "jsonwebtoken"})),
)


def package_names(tree_output: str) -> set[str]:
    """从 `cargo tree --prefix none` 输出提取包名。"""

    names = set()
    for line in tree_output.splitlines():
        match = re.match(r"^([A-Za-z0-9_-]+) v", line.strip())
        if match:
            names.add(match.group(1))
    return names


def violations(case: Case, packages: set[str]) -> list[str]:
    """返回缺失依赖与意外泄漏依赖。"""

    issues = [f"missing required dependency: {name}" for name in sorted(case.required - packages)]
    issues.extend(
        f"unexpected dependency: {name}" for name in sorted(case.forbidden & packages)
    )
    return issues


def run_self_test() -> None:
    """对每条 required/forbidden 规则做恶意删改，证明校验器会拒绝。"""

    for case in CASES:
        valid = set(case.required)
        assert not violations(case, valid)
        for required in case.required:
            assert violations(case, valid - {required})
        for forbidden in case.forbidden:
            assert violations(case, valid | {forbidden})


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        run_self_test()

    failed = False
    for case in CASES:
        command = (
            "cargo",
            "tree",
            "-p",
            case.package,
            *case.args,
            "-e",
            "normal",
            "--prefix",
            "none",
            "--locked",
        )
        result = subprocess.run(command, check=True, capture_output=True, text=True)
        issues = violations(case, package_names(result.stdout))
        if issues:
            failed = True
            for issue in issues:
                print(f"{case.name}: {issue}")
        else:
            print(f"{case.name}: isolated")

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
