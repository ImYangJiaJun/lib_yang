#!/usr/bin/env python3
"""Extract every REST endpoint from the official Lark/Feishu Go SDK.

The SDK is generated from the official API Explorer and is the most complete
machine-readable endpoint catalogue available. Point ``--sdk`` at a shallow
clone of https://github.com/larksuite/oapi-sdk-go.

Usage:
    python -I scripts/extract_openapi_catalog.py \
        --sdk /path/to/lark-oapi-go \
        --out docs/superpowers/specs/2026-11-06-yang-feishu-openapi-catalog.json
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

FUNC = re.compile(
    r"^func \([a-z] \*([A-Za-z0-9_]+)\) ([A-Za-z0-9_]+)"
    r"\(ctx context\.Context, (?:req \*([A-Za-z0-9_]+), )?options \.\.\.larkcore\.RequestOptionFunc\)"
    r" \(\*([A-Za-z0-9_]+), error\) \{"
)
PATH = re.compile(r'apiReq\.ApiPath\s*=\s*"([^"]+)"')
HTTP = re.compile(r"apiReq\.HttpMethod\s*=\s*http\.Method([A-Za-z]+)")
TOKENS = re.compile(r"SupportedAccessTokenTypes\s*=\s*\[\]larkcore\.AccessTokenType\{([^}]*)\}")
LINK = re.compile(r"官网API文档链接:(https://[^\s]+)")
BIN = re.compile(r"resp\.File\s*=\s*bytes\.NewBuffer")
MULTIPART = re.compile(r"WithFileUpload\(\)|FormData")


def extract(sdk: Path) -> list[dict]:
    rows: list[dict] = []
    files = sorted(
        set((sdk / "service").rglob("resource.go"))
        | set((sdk / "service").rglob("api.go"))
    )
    for f in files:
        lines = f.read_text(encoding="utf-8", errors="replace").splitlines()
        i = 0
        while i < len(lines):
            m = FUNC.match(lines[i])
            if not m:
                i += 1
                continue
            c = i - 1
            comments: list[str] = []
            while c >= 0 and lines[c].lstrip().startswith("//"):
                comments.append(lines[c].lstrip()[2:].strip())
                c -= 1
            comments.reverse()
            summary = next((x for x in comments if x and not x.startswith("-")), "")
            link = ""
            for x in comments:
                lm = LINK.search(x)
                if lm:
                    link = lm.group(1)
                    break
            body: list[str] = []
            j = i + 1
            while j < len(lines) and lines[j] != "}":
                body.append(lines[j])
                j += 1
            text = "\n".join(body)
            pm = PATH.search(text)
            hm = HTTP.search(text)
            if pm and hm:
                tm = TOKENS.search(text)
                tokens = (
                    sorted(set(re.findall(r"AccessTokenType([A-Za-z]+)", tm.group(1))))
                    if tm
                    else []
                )
                rows.append(
                    {
                        "domain": f.parent.parent.name,
                        "resource": m.group(1),
                        "go_method": m.group(2),
                        "method": hm.group(1).upper(),
                        "path": pm.group(1),
                        "tokens": tokens,
                        "summary": summary,
                        "doc_url": link,
                        "binary": bool(BIN.search(text)),
                        # `WithFileUpload()` 也用于下载拿原始 body；GET 端点不是
                        # multipart 上传，避免把 spark storage 下载标成 multipart。
                        "multipart": bool(MULTIPART.search(text))
                        and hm.group(1).upper() != "GET",
                    }
                )
            i = j + 1

    seen: set[tuple] = set()
    unique: list[dict] = []
    for row in rows:
        key = (row["method"], row["path"], tuple(row["tokens"]))
        if key in seen:
            continue
        seen.add(key)
        unique.append(row)
    unique.sort(key=lambda r: (r["domain"], r["path"], r["method"], r["tokens"]))
    return unique


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sdk", required=True, help="path to a lark-oapi-go clone")
    parser.add_argument("--out", required=True, help="output JSON path")
    args = parser.parse_args()

    endpoints = extract(Path(args.sdk))
    payload = {
        "source": "https://github.com/larksuite/oapi-sdk-go",
        "description": "Official Feishu/Lark REST endpoints extracted from the Go SDK resource.go files.",
        "endpoints": endpoints,
    }
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {len(endpoints)} endpoints to {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
