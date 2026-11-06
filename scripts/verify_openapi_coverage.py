#!/usr/bin/env python3
"""Verify that every catalogue endpoint is represented by generated Rust code.

This is the safety net for ``scripts/generate_openapi_endpoints.py``: it parses
the generated ``src/api/*.rs`` files, recovers each method's HTTP verb, path and
identity variant, and compares the result with the official endpoint catalogue.

Usage:
    python -I scripts/verify_openapi_coverage.py
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CATALOGS = [
    ROOT / "docs/superpowers/specs/2026-11-06-yang-feishu-openapi-catalog.json",
    ROOT / "docs/superpowers/specs/2026-11-06-yang-feishu-openapi-manual.json",
    ROOT / "docs/superpowers/specs/2026-11-06-yang-feishu-openapi-legacy.json",
]

PATH_LITERAL = re.compile(r'let path\s*=\s*"(/[^"]+)"\s*\.to_string\(\);', re.S)
PATH_FORMAT = re.compile(r'let path\s*=\s*format!\(\s*"([^"]+)"\s*,?\s*\);', re.S)
HTTP_CALL = re.compile(r"Method::([A-Za-z]+)")


def norm(path: str) -> str:
    path = re.sub(r"\{[^}]+\}", ":p", path)
    path = re.sub(r":[A-Za-z0-9_]+", ":p", path)
    return path.rstrip("/")


def variants(tokens: list[str]) -> list[str]:
    tokens = set(tokens)
    out = []
    if "Tenant" in tokens:
        out.append("tenant")
    if "User" in tokens:
        out.append("user")
    if "App" in tokens:
        out.append("app")
    if not out:
        out.append("public")
    return out


def identity(body: str) -> str:
    if "call_raw_json(" in body:
        return "tenant"
    if "call_raw_json_user(" in body:
        return "app" if "app_access_token" in body else "user"
    if "call_json_public(" in body:
        return "public"
    if "call_json_user(" in body:
        return "app" if "app_access_token" in body else "user"
    if "send_raw(" in body:
        if "bearer_token: Some(app_access_token" in body:
            return "app"
        if "bearer_token: Some(bearer_token" in body:
            return "user"
        return "public"
    return "tenant"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--catalog", action="append")
    parser.add_argument("--src", default=str(ROOT / "crates/yang-feishu/src/api"))
    args = parser.parse_args()
    catalogs = [Path(p) for p in args.catalog] if args.catalog else DEFAULT_CATALOGS

    catalog: dict[tuple, dict] = {}
    for path in catalogs:
        data = json.loads(path.read_text(encoding="utf-8"))
        for e in data["endpoints"]:
            key = (norm(e["path"]), e["method"].upper())
            catalog[key] = e

    generated: Counter = Counter()
    files = sorted(Path(args.src).glob("*.rs"))
    for file in files:
        text = file.read_text(encoding="utf-8")
        for block in text.split("    pub async fn ")[1:]:
            body = block.split("\n    }")[0]
            path_match = PATH_LITERAL.search(body) or PATH_FORMAT.search(body)
            if not path_match:
                continue
            http_match = HTTP_CALL.search(body)
            if not http_match:
                continue
            generated[(norm(path_match.group(1)), http_match.group(1).upper(), identity(body))] += 1

    expected: Counter = Counter()
    missing: list[tuple] = []
    for (path, method), endpoint in catalog.items():
        for kind in variants(endpoint.get("tokens") or []):
            key = (path, method, kind)
            expected[key] += 1
            if generated[key] == 0:
                missing.append((endpoint["domain"], method, endpoint["path"], kind))

    extra = [key for key in generated if expected[key] == 0]
    print(f"catalog endpoints={len(catalog)} expected variants={sum(expected.values())}")
    print(f"generated methods={sum(generated.values())} source files={len(files)}")
    print(f"missing variants={len(missing)}")
    for item in missing[:40]:
        print("MISSING", *item)
    print(f"unexpected generated variants={len(extra)}")
    for key in extra[:20]:
        print("EXTRA", key)
    return 1 if missing or extra else 0


if __name__ == "__main__":
    sys.exit(main())
