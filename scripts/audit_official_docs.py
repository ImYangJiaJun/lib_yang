#!/usr/bin/env python3
"""Build a documentation-side OpenAPI catalogue for manual audits.

Fetches every ``open.feishu.cn/llms-docs/zh-CN/llms-*.txt`` bundle, follows the
linked ``.md`` API pages, and extracts the ``HTTP URL`` / ``HTTP Method`` table
plus the ``Authorization`` row of the request-header section.

This script needs network access and is intentionally not part of CI. The
snapshot it writes is the evidence base for
``docs/audit/2026-11-07-yang-feishu-endpoint-audit.md``.

Usage:
    python -I scripts/audit_official_docs.py
    python -I scripts/audit_official_docs.py --out /tmp/doc_catalog.json
"""
from __future__ import annotations

import argparse
import concurrent.futures as cf
import json
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT = ROOT / "docs/superpowers/specs/2026-11-07-yang-feishu-doc-catalog.json"
INDEX = "https://open.feishu.cn/llms-docs/zh-CN/llms.txt"
USER_AGENT = "yang-feishu-doc-audit/1.0"

URL_RE = re.compile(r"HTTP URL\s*\|\s*(https://[^\s|]+)")
METHOD_RE = re.compile(r"HTTP Method\s*\|\s*([A-Z]+)")


def fetch(url: str, timeout: float) -> str:
    """Fetch a UTF-8 document; percent-encode non-ASCII path/query."""
    parts = urllib.parse.urlsplit(url)
    safe = "/%:@!$&'()*+,;=-._~"
    encoded = urllib.parse.urlunsplit(
        (
            parts.scheme,
            parts.netloc,
            urllib.parse.quote(parts.path, safe=safe),
            urllib.parse.quote(parts.query, safe="=&"),
            "",
        )
    )
    request = urllib.request.Request(encoded, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read().decode("utf-8", "replace")


def auth_tokens(text: str) -> list[str]:
    """Return token kinds named in the Authorization row of the 请求头 table."""
    lines = text.splitlines()
    start = None
    for index, line in enumerate(lines):
        if re.match(r"#{2,4}\s*请求头", line):
            start = index
            break
    if start is None:
        return []
    for line in lines[start + 1 :]:
        if line.startswith("### ") or line.startswith("## "):
            break
        if "Authorization" in line and "|" in line:
            names = {
                "tenant_access_token": "Tenant",
                "user_access_token": "User",
                "app_access_token": "App",
            }
            return [kind for token, kind in names.items() if token in line]
    return []


def endpoint_from_page(link: str, text: str) -> dict | None:
    """Parse one doc page; return None for non-API pages."""
    url = URL_RE.search(text)
    method = METHOD_RE.search(text)
    if not url or not method:
        return None
    path = urllib.parse.urlsplit(url.group(1).strip()).path
    title = next(
        (line[2:].strip() for line in text.splitlines() if line.startswith("# ")),
        "",
    )
    return {
        "method": method.group(1).upper(),
        "path": path,
        "ident": auth_tokens(text),
        "title": title,
        "link": link,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--timeout", type=float, default=45.0)
    parser.add_argument("--workers", type=int, default=24)
    args = parser.parse_args()

    index = fetch(INDEX, args.timeout)
    bundles = sorted(
        set(
            re.findall(
                r"\((https://open\.feishu\.cn/llms-docs/zh-CN/llms-[^)\s]+\.txt)\)",
                index,
            )
        )
    )
    print(f"bundles={len(bundles)}", flush=True)

    def get_bundle(url: str) -> tuple[str, str | None, str | None]:
        try:
            return url, fetch(url, args.timeout), None
        except Exception as error:  # noqa: BLE001 - audit script records failures
            return url, None, str(error)

    bundle_texts: dict[str, str] = {}
    with cf.ThreadPoolExecutor(max_workers=12) as pool:
        for url, text, error in pool.map(get_bundle, bundles):
            if text:
                bundle_texts[url] = text
            else:
                print(f"BUNDLE_ERR {url}: {error}", flush=True)

    links: dict[str, set[str]] = {}
    for url, text in bundle_texts.items():
        domain = url.rsplit("llms-", 1)[-1][:-4]
        for link in re.findall(
            r"\((https://open\.feishu\.cn/document/[^)\s]+?\.md)\)", text
        ):
            links.setdefault(link, set()).add(domain)
    print(f"doc_pages={len(links)}", flush=True)

    def get_page(link: str) -> tuple[str, dict | None, str | None]:
        try:
            text = fetch(link, args.timeout)
        except Exception as error:  # noqa: BLE001 - audit script records failures
            return link, None, str(error)
        return link, endpoint_from_page(link, text), None

    rows: list[dict] = []
    errors: list[tuple[str, str]] = []
    no_table = 0
    with cf.ThreadPoolExecutor(max_workers=args.workers) as pool:
        for link, row, error in pool.map(get_page, sorted(links)):
            if error:
                errors.append((link, error))
            elif row:
                row["domains"] = sorted(links[link])
                rows.append(row)
            else:
                no_table += 1

    merged: dict[tuple[str, str], dict] = {}
    for row in rows:
        key = (row["method"], row["path"].rstrip("/"))
        if key not in merged:
            merged[key] = dict(row)
            continue
        current = merged[key]
        current["ident"] = sorted(set(current["ident"]) | set(row["ident"]))
        current["domains"] = sorted(set(current["domains"]) | set(row["domains"]))
        if len(row["title"]) > len(current["title"]):
            current["title"] = row["title"]

    payload = {
        "source": "official llms docs",
        "index": INDEX,
        "pages": len(links),
        "endpoints": [merged[key] for key in sorted(merged)],
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(
        json.dumps(payload, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    error_path = args.out.with_name(args.out.stem + "-errors.json")
    error_path.write_text(
        json.dumps(errors, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    print(
        f"doc_endpoints={len(merged)} no_table={no_table} "
        f"fetch_errors={len(errors)} out={args.out}",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
