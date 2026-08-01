#!/usr/bin/env python3
"""在本地执行与 GitHub Actions 对齐的 lib_yang 质量门禁。"""

from __future__ import annotations

import argparse
import os
import shlex
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Command:
    """一条可执行的质量门禁命令。"""

    name: str
    argv: tuple[str, ...]
    deny_warnings: bool = False


AUXILIARY_PACKAGE_FLAGS = (
    "-p",
    "yang-base-derive",
    "-p",
    "yang-pcg",
    "-p",
    "yang-runtime",
)

STABLE_COMMANDS = (
    Command("CI contract self-test", ("python", "scripts/verify_ci_contract.py", "--self-test")),
    Command(
        "CI contract",
        ("python", "scripts/verify_ci_contract.py", ".github/workflows/ci.yml"),
    ),
    Command(
        "Feature isolation self-test",
        ("python", "scripts/verify_feature_isolation.py", "--self-test"),
    ),
    Command("Formatting", ("cargo", "fmt", "--all", "--", "--check")),
    Command(
        "Performance shadow self-test",
        ("python", "scripts/run_performance_shadow.py", "--self-test"),
    ),
    Command("yang-db library tests", ("cargo", "test", "--lib", "-p", "yang-db", "--locked")),
    Command(
        "yang-base library tests",
        ("cargo", "test", "--lib", "-p", "yang-base", "--locked"),
    ),
    Command(
        "Auxiliary workspace all-target tests",
        (
            "cargo",
            "test",
            "--all-targets",
            *AUXILIARY_PACKAGE_FLAGS,
            "--locked",
        ),
    ),
    Command(
        "Clippy all targets and features",
        (
            "cargo",
            "clippy",
            "-p",
            "yang-db",
            "-p",
            "yang-base",
            *AUXILIARY_PACKAGE_FLAGS,
            "--all-targets",
            "--all-features",
            "--locked",
            "--",
            "-D",
            "warnings",
        ),
    ),
    Command("yang-db documentation tests", ("cargo", "test", "--doc", "-p", "yang-db", "--locked")),
    Command(
        "yang-base documentation tests",
        ("cargo", "test", "--doc", "-p", "yang-base", "--locked"),
    ),
    Command(
        "yang-runtime documentation tests",
        ("cargo", "test", "--doc", "-p", "yang-runtime", "--locked"),
    ),
)

SUPPLY_CHAIN_COMMANDS = (
    Command(
        "Dependency policy self-test",
        ("python", "scripts/verify_dependency_policy.py", "--self-test"),
    ),
    Command("Dependency policy", ("python", "scripts/verify_dependency_policy.py")),
    Command(
        "Rust dependency audit",
        (
            "cargo",
            "deny",
            "--all-features",
            "--locked",
            "check",
            "advisories",
            "licenses",
            "sources",
        ),
    ),
)

MSRV_COMMAND = Command(
    "MSRV 1.80",
    (
        "cargo",
        "+1.80.0",
        "check",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--locked",
    ),
)

FEATURE_CASES = (
    ("yang-db-none", "yang-db", ("--no-default-features",)),
    ("yang-db-mysql", "yang-db", ("--no-default-features", "--features", "mysql")),
    ("yang-db-postgres", "yang-db", ("--no-default-features", "--features", "postgres")),
    ("yang-db-redis", "yang-db", ("--no-default-features", "--features", "redis")),
    ("yang-db-all", "yang-db", ("--all-features",)),
    ("yang-base-none", "yang-base", ("--no-default-features",)),
    ("yang-base-token", "yang-base", ("--no-default-features", "--features", "token")),
    ("yang-base-http", "yang-base", ("--no-default-features", "--features", "http")),
    ("yang-base-mysql", "yang-base", ("--no-default-features", "--features", "mysql")),
    ("yang-base-redis", "yang-base", ("--no-default-features", "--features", "redis")),
    ("yang-base-validator", "yang-base", ("--no-default-features", "--features", "validator")),
    (
        "yang-base-plugin-schema",
        "yang-base",
        ("--no-default-features", "--features", "plugin-schema"),
    ),
    ("yang-base-metrics", "yang-base", ("--no-default-features", "--features", "metrics")),
    ("yang-base-openapi", "yang-base", ("--no-default-features", "--features", "openapi")),
    (
        "yang-base-admin-metadata",
        "yang-base",
        ("--no-default-features", "--features", "admin-metadata"),
    ),
    ("yang-base-default", "yang-base", ()),
    ("yang-base-all", "yang-base", ("--all-features",)),
)

INTEGRATION_COMMANDS = (
    Command(
        "Typed Action integration",
        (
            "cargo",
            "test",
            "-p",
            "yang-base",
            "--test",
            "typed_action_integration",
            "--locked",
            "--",
            "--ignored",
            "--test-threads=1",
        ),
    ),
    Command(
        "TableQuery CRUD integration",
        (
            "cargo",
            "test",
            "-p",
            "yang-base",
            "--test",
            "table_query_crud_test",
            "--locked",
            "--",
            "--ignored",
            "--test-threads=1",
        ),
    ),
    Command(
        "TableQuery pagination integration",
        (
            "cargo",
            "test",
            "-p",
            "yang-base",
            "--test",
            "table_query_paginate_test",
            "--locked",
            "--",
            "--ignored",
            "--test-threads=1",
        ),
    ),
    Command(
        "TableQuery transaction integration",
        (
            "cargo",
            "test",
            "-p",
            "yang-base",
            "--test",
            "table_query_transaction_test",
            "--locked",
            "--",
            "--ignored",
            "--test-threads=1",
        ),
    ),
    Command(
        "PostgreSQL CRUD integration",
        (
            "cargo",
            "test",
            "-p",
            "yang-db",
            "--test",
            "integration_pg_crud_simple",
            "--locked",
            "--",
            "--ignored",
            "--test-threads=1",
        ),
    ),
    Command(
        "PostgreSQL transaction integration",
        (
            "cargo",
            "test",
            "-p",
            "yang-db",
            "--test",
            "integration_pg_transaction",
            "--locked",
            "--",
            "--ignored",
            "--test-threads=1",
        ),
    ),
    Command(
        "Redis pipeline integration",
        (
            "cargo",
            "test",
            "-p",
            "yang-db",
            "--test",
            "integration_redis_pipeline",
            "--locked",
            "--",
            "--test-threads=1",
        ),
    ),
    Command(
        "Token revocation integration",
        (
            "cargo",
            "test",
            "-p",
            "yang-base",
            "--test",
            "token_revocation_integration",
            "--locked",
            "--",
            "--ignored",
            "--test-threads=1",
        ),
    ),
    Command(
        "Redis script integration",
        (
            "cargo",
            "test",
            "-p",
            "yang-db",
            "--test",
            "integration_redis_script",
            "--locked",
            "--",
            "--test-threads=1",
        ),
    ),
)


def feature_commands() -> tuple[Command, ...]:
    """生成与 CI matrix 一致的 check/test/doctest 命令。"""

    commands = []
    for name, package, args in FEATURE_CASES:
        prefix = ("cargo",)
        suffix = ("-p", package, *args, "--locked")
        commands.extend(
            (
                Command(f"{name}: check", (*prefix, "check", *suffix), True),
                Command(f"{name}: test", (*prefix, "test", "--lib", *suffix), True),
                Command(f"{name}: doctest", (*prefix, "test", "--doc", *suffix), True),
            )
        )
    return tuple(commands)


def run(command: Command) -> None:
    """执行一条命令，失败时立即终止。"""

    argv = list(command.argv)
    if argv[0] == "python":
        argv[0] = sys.executable
    env = os.environ.copy()
    if command.deny_warnings:
        env["RUSTFLAGS"] = "-Dwarnings"
        env["RUSTDOCFLAGS"] = "-Dwarnings"
    print(f"\n==> {command.name}\n    {shlex.join(argv)}", flush=True)
    subprocess.run(argv, check=True, env=env)


def self_test() -> None:
    """验证本地入口覆盖 CI 的关键组合和锁文件约束。"""

    toolchain = tomllib.loads(Path("rust-toolchain.toml").read_text(encoding="utf-8"))[
        "toolchain"
    ]
    channel = toolchain["channel"]
    assert {"clippy", "rustfmt"}.issubset(toolchain["components"])
    workflow = Path(".github/workflows/ci.yml").read_text(encoding="utf-8")
    assert f'CI_RUST_VERSION: "{channel}"' in workflow

    expected_features = {
        "yang-db-none",
        "yang-db-mysql",
        "yang-db-postgres",
        "yang-db-redis",
        "yang-db-all",
        "yang-base-none",
        "yang-base-token",
        "yang-base-http",
        "yang-base-mysql",
        "yang-base-redis",
        "yang-base-validator",
        "yang-base-plugin-schema",
        "yang-base-metrics",
        "yang-base-openapi",
        "yang-base-admin-metadata",
        "yang-base-default",
        "yang-base-all",
    }
    actual_features = {name for name, _, _ in FEATURE_CASES}
    assert actual_features == expected_features
    assert len(feature_commands()) == len(FEATURE_CASES) * 3
    workspace_manifest = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))
    workspace_msrv = workspace_manifest["workspace"]["package"]["rust-version"]
    assert MSRV_COMMAND.argv == (
        "cargo",
        f"+{workspace_msrv}.0",
        "check",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--locked",
    )
    member_manifests = tuple(Path("crates").glob("*/Cargo.toml"))
    member_packages = [
        tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]
        for manifest in member_manifests
    ]
    assert all(
        package.get("rust-version") == {"workspace": True} for package in member_packages
    ), "所有工作区成员都必须显式继承 workspace.package.rust-version"
    workspace_packages = {package["name"] for package in member_packages}
    tested_packages = set()
    clippy_packages = set()
    for command in STABLE_COMMANDS:
        if command.argv[:2] == ("cargo", "test"):
            tested_packages.update(
                command.argv[index + 1]
                for index, argument in enumerate(command.argv[:-1])
                if argument == "-p"
            )
        if command.argv[:2] == ("cargo", "clippy"):
            clippy_packages.update(
                command.argv[index + 1]
                for index, argument in enumerate(command.argv[:-1])
                if argument == "-p"
            )
    assert tested_packages == workspace_packages
    assert clippy_packages == workspace_packages
    assert SUPPLY_CHAIN_COMMANDS[-1].argv == (
        "cargo",
        "deny",
        "--all-features",
        "--locked",
        "check",
        "advisories",
        "licenses",
        "sources",
    )
    for command in (
        *STABLE_COMMANDS[4:],
        *SUPPLY_CHAIN_COMMANDS,
        MSRV_COMMAND,
        *feature_commands(),
        *INTEGRATION_COMMANDS,
    ):
        if command.argv[0] == "cargo":
            assert "--locked" in command.argv, f"命令缺少 --locked: {command.name}"
    print("local CI runner self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile", nargs="?", choices=("quick", "full", "integration"), default="quick")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0

    if args.profile == "integration":
        commands = INTEGRATION_COMMANDS
    elif args.profile == "full":
        commands = (*STABLE_COMMANDS, *SUPPLY_CHAIN_COMMANDS, MSRV_COMMAND, *feature_commands())
    else:
        commands = STABLE_COMMANDS

    for command in commands:
        run(command)
    print(f"\nCI profile passed: {args.profile}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
