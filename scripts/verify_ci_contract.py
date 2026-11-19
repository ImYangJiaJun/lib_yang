#!/usr/bin/env python3
"""校验基础库 CI 是否保留计划要求的关键门禁。"""

from __future__ import annotations

import argparse
import re
import shlex
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
    'CI_RUST_VERSION: "1.100.0"',
    "toolchain: ${{ env.CI_RUST_VERSION }}",
    "cargo fmt --all -- --check",
    "cargo test --lib -p yang-db --locked",
    "cargo test --lib -p yang-base --locked",
    "cargo test -p yang-base --test compatibility_contract --test error_test --test field_type_test --test observability_test --test plugin_test --test schema_snapshots --test database_test --locked",
    "cargo test -p yang-base --test release_candidate_contract --test release_docs_contract --locked",
    "cargo test -p yang-base --features admin-metadata --test admin_metadata_contract --locked",
    "cargo test -p yang-base --features transport-axum --test transport_axum --locked",
    "cargo test -p yang-base --features transport-axum,metrics --test transport_inbound_observability --locked",
    "cargo test --all-targets -p yang-base-derive -p yang-pcg -p yang-runtime --locked",
    "cargo test --all-targets -p yang-feishu --all-features --locked",
    "cargo clippy -p yang-db -p yang-base -p yang-base-derive -p yang-pcg -p yang-runtime -p yang-feishu --all-targets --all-features --locked -- -D warnings",
    "cargo test --doc -p yang-db --locked",
    "cargo test --doc -p yang-base --locked",
    "cargo test --doc -p yang-runtime --locked",
    "cargo test --doc -p yang-feishu --locked",
    "python scripts/verify_feature_isolation.py --self-test",
    "python scripts/verify_openapi_coverage.py",
    "python scripts/verify_ci_contract.py --self-test",
    "python scripts/verify_ci_contract.py .github/workflows/ci.yml",
    "python scripts/run_ci.py --self-test",
    "python scripts/run_performance_shadow.py --self-test",
    "python scripts/verify_dependency_policy.py --self-test",
    "python scripts/verify_dependency_policy.py",
    "EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25",
    "actions/checkout@11d5960a326750d5838078e36cf38b85af677262",
    "dtolnay/rust-toolchain@df2a88e9fe4098bad09cb42c3dec7187f15861fc",
    "Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0",
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
    "cargo test -p yang-base --test schema_sync_integration_test --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_record_pg --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_schema_sync_pg --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_table_query_pg --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --no-default-features --features postgres --test integration_table_query_pg_phase_c --locked -- --ignored --test-threads=1",
    'PG_TEST_URL: postgres://postgres:postgres@localhost:5432/test',
    "performance-shadow:",
    "runs-on: ubuntu-24.04",
    "continue-on-error: true",
    "python scripts/run_performance_shadow.py --output \"${{ runner.temp }}/performance-shadow\"",
    "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9",
    'REQUIRE_DOCKER_TESTS: "1"',
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
    "cargo test -p yang-base --no-default-features --features postgres --test integration_pg_wave5_returning --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --no-default-features --features postgres --test integration_pg_wave5_expressions --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --features mysql --test test_aggregate_combinations --locked -- --ignored --test-threads=1",
    "cargo test -p yang-db --test integration_database --locked -- --ignored --test-threads=1",
    "cargo test -p yang-base --test schema_sync_integration_test --locked -- --ignored --test-threads=1",
    "--test-threads=1",
)


def strip_yaml_comments(text: str) -> str:
    """去掉 YAML 注释：注释行不能替代真实的门禁命令。"""

    visible_lines: list[str] = []
    for raw_line in text.splitlines():
        visible: list[str] = []
        quote: str | None = None
        index = 0
        while index < len(raw_line):
            char = raw_line[index]
            if quote is not None:
                visible.append(char)
                if char == "\\" and quote == '"' and index + 1 < len(raw_line):
                    index += 1
                    visible.append(raw_line[index])
                elif char == quote:
                    quote = None
            elif char in ("'", '"'):
                quote = char
                visible.append(char)
            elif char == "#" and (index == 0 or raw_line[index - 1].isspace()):
                break
            else:
                visible.append(char)
            index += 1
        visible_lines.append("".join(visible).rstrip())
    return "\n".join(visible_lines)


def missing_contract_fragments(text: str) -> list[str]:
    """返回 CI 文本中缺失的必需契约片段（忽略 YAML 注释）。"""

    visible = strip_yaml_comments(text)
    return [fragment for fragment in REQUIRED_FRAGMENTS if fragment not in visible]


def run_ci_module():
    """导入 scripts/run_ci.py，作为 CI 矩阵与 integration 目标的唯一事实源。"""

    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import run_ci

    return run_ci


def integration_test_targets() -> tuple[str, ...]:
    """从本地 runner 的 integration 命令提取 --test 目标。"""

    targets = []
    for command in run_ci_module().INTEGRATION_COMMANDS:
        argv = command.argv
        for index, argument in enumerate(argv[:-1]):
            if argument == "--test":
                targets.append(argv[index + 1])
    return tuple(dict.fromkeys(targets))


def missing_integration_targets(text: str) -> list[str]:
    """返回 workflow 文本中缺失的本地 integration 目标（忽略 YAML 注释）。"""

    visible = strip_yaml_comments(text)
    present = set(re.findall(r"--test\s+(\S+)", visible))
    # Redis 命令套件在 workflow 中用 `for suite in ...; do` 循环执行。
    for group in re.findall(r"for suite in ([^;]+);\s*do", visible):
        present.update(token.strip("\"'") for token in group.split())
    return [target for target in integration_test_targets() if target not in present]


def _job_block(text: str, job_name: str) -> str | None:
    """提取 workflow 中指定 job 的文本块（不含 job 名行）。"""

    lines = text.splitlines()
    start = None
    for index, line in enumerate(lines):
        if re.match(rf"^  {re.escape(job_name)}:\s*(?:#.*)?$", line):
            start = index
            break
    if start is None:
        return None
    block: list[str] = []
    for line in lines[start + 1 :]:
        if re.match(r"^  [A-Za-z0-9_.-]+:\s*(?:#.*)?$", line):
            break
        block.append(line)
    return "\n".join(block)


def _yaml_scalar(raw: str) -> str:
    """还原 YAML 标量的引号，保持与运行值一致的文本。"""

    value = raw.strip()
    if len(value) >= 2 and value[0] == value[-1] == '"':
        return value[1:-1].replace('\\"', '"')
    if len(value) >= 2 and value[0] == value[-1] == "'":
        return value[1:-1].replace("''", "'")
    return value


def _args_tokens(raw: str) -> tuple[str, ...]:
    """把矩阵 args 归一化为 shell token；空串与缺省写法等价。"""

    value = _yaml_scalar(raw)
    if not value.strip():
        return ()
    tokens = tuple(shlex.split(value))
    normalized: list[str] = []
    for token in tokens:
        if token.startswith("--features=") and token != "--features=":
            normalized.extend(("--features", token.split("=", 1)[1]))
        else:
            normalized.append(token)
    return tuple(normalized)


def parse_feature_matrix(text: str) -> list[dict[str, str]] | None:
    """解析 feature-matrix job 的 include 条目，容忍缩进/引号/空 args。"""

    block = _job_block(text, "feature-matrix")
    if block is None:
        return None

    entries: list[dict[str, str]] = []
    current: dict[str, str] | None = None
    seen_include = False
    include_indent = 0
    for raw_line in block.splitlines():
        line = strip_yaml_comments(raw_line).rstrip()
        if not line.strip():
            continue
        stripped = line.strip()
        indent = len(line) - len(line.lstrip(" "))
        if not seen_include:
            if stripped == "include:":
                seen_include = True
                include_indent = indent
            continue
        if indent <= include_indent and not stripped.startswith("- "):
            break
        if stripped.startswith("- "):
            if current is not None:
                entries.append(current)
            item = stripped[2:].strip()
            current = {}
            if item.startswith("{"):
                for key in ("name", "package", "args"):
                    match = re.search(rf"\b{key}\s*:\s*(\"[^\"]*\"|'[^']*'|[^,}}]*)", item)
                    if match:
                        current[key] = match.group(1).strip()
            elif item and ":" in item:
                key, value = item.split(":", 1)
                current[key.strip()] = value.strip()
            continue
        if current is not None and ":" in stripped:
            key, value = stripped.split(":", 1)
            current[key.strip()] = value.strip()
    if current is not None:
        entries.append(current)
    return entries


def feature_matrix_violations(text: str) -> list[str]:
    """校验矩阵的 name/package/args 关联与 run_ci.FEATURE_CASES 完全一致。"""

    entries = parse_feature_matrix(text)
    if entries is None:
        return ["feature-matrix: workflow 中未找到 feature-matrix job"]
    if not entries:
        return ["feature-matrix: 未能解析 include 条目（请使用逐条 include 写法）"]

    actual: dict[str, tuple[str, tuple[str, ...]]] = {}
    violations: list[str] = []
    for entry in entries:
        name = _yaml_scalar(entry.get("name", ""))
        if not name:
            violations.append("feature-matrix: 存在缺少 name 的矩阵条目")
            continue
        if name in actual:
            violations.append(f"feature-matrix: 组合重复: {name}")
            continue
        package = _yaml_scalar(entry.get("package", ""))
        actual[name] = (package, _args_tokens(entry.get("args", "")))

    expected = {
        name: (package, tuple(args))
        for name, package, args in run_ci_module().FEATURE_CASES
    }
    for name in sorted(expected):
        if name not in actual:
            violations.append(f"feature-matrix: 缺少组合: {name}")
            continue
        expected_package, expected_args = expected[name]
        actual_package, actual_args = actual[name]
        if actual_package != expected_package or actual_args != expected_args:
            violations.append(
                f"feature-matrix: 组合不一致: {name} "
                f"(期望 package={expected_package} args={expected_args!r}，"
                f"实际 package={actual_package} args={actual_args!r})"
            )
    for name in sorted(set(actual) - set(expected)):
        violations.append(f"feature-matrix: 多出未登记组合: {name}")
    return violations


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


MYSQL_TEST_FILES = (
    "crates/yang-base/tests/typed_action_integration.rs",
    "crates/yang-base/tests/table_query_crud_test.rs",
    "crates/yang-base/tests/table_query_paginate_test.rs",
    "crates/yang-base/tests/table_query_transaction_test.rs",
    "crates/yang-base/tests/schema_sync_integration_test.rs",
)

MYSQL_DOCKER_GATE_MARKERS = (
    "REQUIRE_DOCKER_TESTS",
    "YANG_REQUIRE_DOCKER",
)

# fail-closed 必须是「由门控变量控制的 panic/expect」，而不是文件里任意
# 一处无关的 `.expect(`（后者会让删除门控分支的回归静默通过）。
MYSQL_FAIL_CLOSED_PATTERN = re.compile(
    r"if\s+[A-Za-z_][A-Za-z0-9_]*\s*\{[\s\S]{0,600}?(?:panic!|\.expect\()"
)

MYSQL_UNGATED_SOFT_SKIP_PATTERNS = (
    re.compile(r'println!\("跳过测试'),
    re.compile(r'println!\("跳过集成测试'),
    re.compile(r"\.ok\(\)\?"),
    re.compile(r"is_err\(\)\s*\{\s*return\s*;", re.S),
)


def text_mysql_soft_skip_violations(text: str, name: str) -> list[str]:
    """检查单个 MySQL 集成测试是否仍存在无门控的「失败即跳过」。"""

    violations: list[str] = []
    if not any(marker in text for marker in MYSQL_DOCKER_GATE_MARKERS):
        violations.append(
            f"{name}: 未使用 REQUIRE_DOCKER_TESTS/YANG_REQUIRE_DOCKER 环境门控"
            "（CI 仍可能零断言变绿）"
        )
    if not MYSQL_FAIL_CLOSED_PATTERN.search(text):
        violations.append(
            f"{name}: 未发现由环境门控变量控制的 panic/expect fail-closed 分支"
        )
    for pattern in MYSQL_UNGATED_SOFT_SKIP_PATTERNS:
        if pattern.search(text):
            violations.append(f"{name}: 仍包含无门控软跳过模式 {pattern.pattern!r}")
            break
    return violations


def mysql_soft_skip_violations(root: Path) -> list[str]:
    """检查 CI 实际运行的四个 MySQL target 是否仍可能静默通过。"""

    violations: list[str] = []
    for relative in MYSQL_TEST_FILES:
        path = root / relative
        if not path.exists():
            violations.append(f"{path}: MySQL 集成测试文件缺失")
            continue
        text = path.read_text(encoding="utf-8")
        violations.extend(text_mysql_soft_skip_violations(text, relative))
    return violations


def feature_matrix_fixture(cases: tuple[tuple[str, str, tuple[str, ...]], ...]) -> str:
    """按 workflow 的 include 形状生成矩阵 fixture（供 self-test 使用）。"""

    lines = [
        "  feature-matrix:",
        "    strategy:",
        "      matrix:",
        "        include:",
    ]
    for name, package, args in cases:
        lines.append(f"          - name: {name}")
        lines.append(f"            package: {package}")
        if args:
            lines.append(f"            args: {' '.join(args)}")
        else:
            lines.append('            args: ""')
    return "\n".join(lines)


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

    # F136：整行注释不能替代真实命令；必须存在校验器的两条自调用。
    commented_fixture = "\n".join(f"# {fragment}" for fragment in REQUIRED_FRAGMENTS)
    commented_missing = missing_contract_fragments(commented_fixture)
    assert len(commented_missing) == len(REQUIRED_FRAGMENTS), (
        "注释行被错误地当作有效 CI 契约"
    )
    assert not missing_contract_fragments(valid_fixture + "\n# note: stable:")
    assert not missing_contract_fragments(
        valid_fixture.replace("stable:", "stable: # required")
    )

    # F131：feature-matrix 的 name/package/args 必须与 run_ci.FEATURE_CASES 一致。
    cases = run_ci_module().FEATURE_CASES
    matrix_fixture = feature_matrix_fixture(cases)
    assert not feature_matrix_violations(matrix_fixture), feature_matrix_violations(
        matrix_fixture
    )
    assert not feature_matrix_violations(feature_matrix_fixture(tuple(reversed(cases))))
    assert not feature_matrix_violations(
        matrix_fixture.replace("args: --no-default-features", 'args: "--no-default-features"', 1)
    )
    assert not feature_matrix_violations(
        matrix_fixture.replace('args: ""', "args:", 1)
    )

    first_case = (
        "          - name: yang-db-none\n"
        "            package: yang-db\n"
        "            args: --no-default-features"
    )
    assert not feature_matrix_violations(
        matrix_fixture.replace(
            first_case,
            "          - package: yang-db\n"
            "            args: --no-default-features\n"
            "            name: yang-db-none",
        )
    ), "矩阵键顺序变化不应导致违约"
    assert not feature_matrix_violations(
        matrix_fixture.replace(
            first_case,
            "          - {name: yang-db-none, package: yang-db,"
            " args: --no-default-features}",
        )
    ), "矩阵行内写法不应导致违约"
    assert not feature_matrix_violations(
        matrix_fixture.replace(
            "args: --no-default-features --features mysql",
            "args: --no-default-features --features=mysql",
            1,
        )
    ), "--features=value 与 --features value 应等价"

    tampered_args = list(cases)
    target_name, target_package, target_args = tampered_args[1]
    tampered_args[1] = (
        target_name,
        target_package,
        (*target_args, "--features", "redis"),
    )
    assert feature_matrix_violations(feature_matrix_fixture(tampered_args)), (
        "未检测到 feature-matrix args 篡改"
    )

    tampered_package = list(cases)
    name, package, args = tampered_package[0]
    tampered_package[0] = (name, "yang-wrong", args)
    assert feature_matrix_violations(feature_matrix_fixture(tampered_package)), (
        "未检测到 feature-matrix package 篡改"
    )

    assert feature_matrix_violations(feature_matrix_fixture(cases[:-1])), (
        "未检测到缺失的 feature-matrix 组合"
    )
    assert feature_matrix_violations(
        feature_matrix_fixture((*cases, ("yang-extra", "yang-base", ())))
    ), "未检测到多余的 feature-matrix 组合"

    # F029/F115：MySQL 套件必须保留门控，且不能有无门控的失败即跳过。
    malicious_mysql = """
    async fn setup_mysql() -> Option<()> {
        let container = match image.start().await {
            Ok(container) => container,
            Err(error) => {
                println!("跳过测试：无法启动 Docker 容器: {}", error);
                return None;
            }
        };
        let port = container.get_host_port_ipv4(3306).await.ok()?;
        None
    }
    """
    assert text_mysql_soft_skip_violations(malicious_mysql, "<malicious-mysql>"), (
        "MySQL soft-skip 检测器未能识别无门控跳过"
    )

    clean_mysql = """
    const REQUIRE_DOCKER_ENV: &str = "REQUIRE_DOCKER_TESTS";
    fn skip_or_fail(require_docker: bool, reason: String) -> Option<()> {
        if require_docker {
            panic!("MySQL 集成测试要求可用的 Docker/MySQL（fail-closed）：{reason}");
        }
        eprintln!("本地跳过 MySQL 集成测试：{reason}");
        None
    }
    """
    assert not text_mysql_soft_skip_violations(clean_mysql, "<clean-mysql>"), (
        "MySQL soft-skip 检测器误报了带门控的本地跳过"
    )

    clean_mysql_expect = """
    const REQUIRE_DOCKER_ENV: &str = "REQUIRE_DOCKER_TESTS";
    fn setup_mysql() -> Option<()> {
        let required = std::env::var(REQUIRE_DOCKER_ENV).is_ok();
        if required {
            let _container = image.start().expect("MySQL 容器必须可启动");
        }
        None
    }
    """
    assert not text_mysql_soft_skip_violations(clean_mysql_expect, "<clean-mysql-expect>"), (
        "MySQL soft-skip 检测器误报了 expect fail-closed 形态"
    )

    # 负向回归：删除门控分支后不能靠文件里无关的 .expect( 继续通过。
    gate_removed = re.sub(
        r"if require_docker \{[\s\S]*?panic![\s\S]*?\}", "", clean_mysql
    )
    assert text_mysql_soft_skip_violations(gate_removed, "<gate-removed>"), (
        "MySQL soft-skip 检测器未识别被删除的 fail-closed 门控分支"
    )
    unrelated_expect = """
    const REQUIRE_DOCKER_ENV: &str = "REQUIRE_DOCKER_TESTS";
    fn setup() {
        let _ = Database::connect("mysql://localhost/test").expect("连接");
    }
    """
    assert text_mysql_soft_skip_violations(unrelated_expect, "<unrelated-expect>"), (
        "MySQL soft-skip 检测器把无关的 .expect( 当成了 fail-closed 门控"
    )

    # F136：合法 YAML 行尾注释不能让 job 解析器误判为缺失。
    commented_job = feature_matrix_fixture(run_ci_module().FEATURE_CASES).replace(
        "  feature-matrix:", "  feature-matrix: # matrix", 1
    )
    assert not feature_matrix_violations(commented_job), (
        "job 行尾注释导致了错误的 feature-matrix 缺失判定"
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
    matrix_violations = feature_matrix_violations(text)
    if missing or missing_targets or matrix_violations:
        for fragment in missing:
            print(f"missing CI contract fragment: {fragment}")
        for target in missing_targets:
            print(f"missing local integration target in CI: {target}")
        for violation in matrix_violations:
            print(f"feature-matrix contract violation: {violation}")
        return 1

    violations = [
        *soft_skip_violations(Path.cwd()),
        *mysql_soft_skip_violations(Path.cwd()),
    ]
    if violations:
        for violation in violations:
            print(f"soft-skip violation: {violation}")
        return 1

    print(f"CI contract verified: {args.workflow}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
