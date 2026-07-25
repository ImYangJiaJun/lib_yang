#!/usr/bin/env python3
"""验证依赖告警例外具备边界、复核期限和自动退出条件。"""

from __future__ import annotations

import argparse
import re
import subprocess
import tomllib
from datetime import date, timedelta
from pathlib import Path
from typing import Any


ADVISORY_ID = re.compile(r"^RUSTSEC-\d{4}-\d{4}$")
REQUIRED_REASON_FIELDS = frozenset({"scope", "review-by", "exit"})
MAX_REVIEW_WINDOW = timedelta(days=180)
RFC2822_TYPE = "time::format_description::well_known::Rfc2822"


def parse_reason(reason: str) -> dict[str, str]:
    """解析 ``key=value; ...`` 形式的机器可校验例外理由。"""

    fields: dict[str, str] = {}
    for clause in reason.split(";"):
        key, separator, value = clause.strip().partition("=")
        if not separator or not key or not value.strip():
            raise ValueError(f"例外理由必须使用非空 key=value 子句: {clause!r}")
        if key in fields:
            raise ValueError(f"例外理由字段重复: {key}")
        fields[key] = value.strip()
    missing = REQUIRED_REASON_FIELDS - fields.keys()
    if missing:
        raise ValueError(f"例外理由缺少字段: {', '.join(sorted(missing))}")
    return fields


def validate_exceptions(exceptions: Any, today: date) -> list[str]:
    """返回依赖例外配置中的全部错误。"""

    errors: list[str] = []
    if not isinstance(exceptions, list):
        return ["advisories.ignore 必须是数组"]

    seen: set[str] = set()
    for index, exception in enumerate(exceptions):
        prefix = f"advisories.ignore[{index}]"
        if not isinstance(exception, dict):
            errors.append(f"{prefix} 必须是包含 id/reason 的表")
            continue
        advisory_id = exception.get("id")
        reason = exception.get("reason")
        if not isinstance(advisory_id, str) or not ADVISORY_ID.fullmatch(advisory_id):
            errors.append(f"{prefix}.id 不是有效 RUSTSEC 编号")
            continue
        if advisory_id in seen:
            errors.append(f"{prefix}.id 重复: {advisory_id}")
        seen.add(advisory_id)
        if not isinstance(reason, str):
            errors.append(f"{prefix}.reason 必须是字符串")
            continue
        try:
            fields = parse_reason(reason)
            review_by = date.fromisoformat(fields["review-by"])
        except (KeyError, ValueError) as error:
            errors.append(f"{prefix}.reason 无效: {error}")
            continue
        if review_by <= today:
            errors.append(f"{advisory_id} 已到复核日: {review_by.isoformat()}")
        if review_by - today > MAX_REVIEW_WINDOW:
            errors.append(
                f"{advisory_id} 复核窗口超过 {MAX_REVIEW_WINDOW.days} 天: "
                f"{review_by.isoformat()}"
            )
    return errors


def production_dependency_graph() -> str:
    """返回不含 dev 边的工作区依赖图。"""

    result = subprocess.run(
        (
            "cargo",
            "tree",
            "--edges",
            "normal,build",
            "--workspace",
            "--all-features",
            "--locked",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ),
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout


def verify_repository() -> list[str]:
    """验证当前仓库的依赖策略与风险边界。"""

    errors: list[str] = []
    policy = tomllib.loads(Path("deny.toml").read_text(encoding="utf-8"))
    advisories = policy.get("advisories", {})
    errors.extend(validate_exceptions(advisories.get("ignore"), date.today()))
    if advisories.get("unused-ignored-advisory") != "deny":
        errors.append("advisories.unused-ignored-advisory 必须为 deny")

    clippy = tomllib.loads(Path("clippy.toml").read_text(encoding="utf-8"))
    if RFC2822_TYPE not in clippy.get("disallowed-types", []):
        errors.append(f"clippy.toml 必须禁止 {RFC2822_TYPE}")

    graph = production_dependency_graph()
    if any(line.startswith("rustls-pemfile v") for line in graph.splitlines()):
        errors.append("rustls-pemfile 重新进入 production/build 依赖图")

    root_manifest = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))
    if "rsa" in root_manifest.get("workspace", {}).get("dependencies", {}):
        errors.append("workspace 新增了直接 rsa 依赖，必须重新评审 RUSTSEC-2023-0071")
    return errors


def self_test() -> None:
    """用正反例证明例外的格式、期限与生产图检查会 fail-closed。"""

    today = date(2026, 7, 26)
    valid = [
        {
            "id": "RUSTSEC-2026-0009",
            "reason": (
                "scope=fixed parser path; review-by=2026-10-31; "
                "exit=dependency path changes"
            ),
        }
    ]
    assert not validate_exceptions(valid, today)

    missing_exit = [{"id": valid[0]["id"], "reason": "scope=x; review-by=2026-10-31"}]
    assert any("缺少字段" in error for error in validate_exceptions(missing_exit, today))

    expired = [
        {
            "id": valid[0]["id"],
            "reason": "scope=x; review-by=2026-07-26; exit=y",
        }
    ]
    assert any("已到复核日" in error for error in validate_exceptions(expired, today))

    too_distant = [
        {
            "id": valid[0]["id"],
            "reason": "scope=x; review-by=2027-07-26; exit=y",
        }
    ]
    assert any("复核窗口超过" in error for error in validate_exceptions(too_distant, today))

    duplicate = [valid[0], valid[0]]
    assert any(".id 重复" in error for error in validate_exceptions(duplicate, today))
    assert any(
        line.startswith("rustls-pemfile v")
        for line in "rustls-pemfile v2.2.0\nreqwest v0.12.16\n".splitlines()
    )
    print("dependency policy self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0

    try:
        errors = verify_repository()
    except (OSError, subprocess.CalledProcessError, tomllib.TOMLDecodeError) as error:
        print(f"dependency policy verification failed: {error}")
        return 1
    if errors:
        for error in errors:
            print(f"dependency policy violation: {error}")
        return 1
    print("dependency policy verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
