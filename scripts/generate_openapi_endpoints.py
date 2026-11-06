#!/usr/bin/env python3
"""Generate thin Rust wrappers for every official Feishu/Lark REST endpoint.

Inputs:
  docs/superpowers/specs/2026-11-06-yang-feishu-openapi-catalog.json  (SDK extract)
  docs/superpowers/specs/2026-11-06-yang-feishu-openapi-manual.json   (llms-only extras)

Output:
  crates/yang-feishu/src/api/mod.rs
  crates/yang-feishu/src/api/<domain>.rs

The generated code intentionally stays thin: explicit path parameters,
`&[(&str, &str)]` for query parameters, and `serde_json::Value` bodies.
Typed business flows remain in the hand-written clients.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

RUST_KEYWORDS = {
    "as", "break", "const", "continue", "crate", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
    "super", "trait", "true", "type", "unsafe", "use", "where", "while", "async",
    "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
}
RESERVED_ARGS = {
    "self", "query", "body", "url", "path", "method", "request", "response",
    "result", "core", "token", "value", "name", "type", "match", "move",
    "ref", "box", "async", "await", "dyn", "fn", "let",
}
METHOD_TITLE = {"GET": "Get", "POST": "Post", "PUT": "Put", "PATCH": "Patch", "DELETE": "Delete"}


def snake(value: str) -> str:
    value = re.sub(r"(?<!^)(?=[A-Z])", "_", value)
    value = re.sub(r"[^A-Za-z0-9_]", "_", value)
    value = re.sub(r"_+", "_", value).strip("_").lower()
    return value


def clean_summary(value: str) -> str:
    value = re.sub(r"<[^>]+>", "", value or "")
    value = re.sub(r"\s+", " ", value).strip()
    if len(value) > 180:
        value = value[:177].rstrip() + "..."
    return value


def load_endpoints(paths: list[Path]) -> list[dict]:
    merged: dict[tuple, dict] = {}
    for path in paths:
        data = json.loads(path.read_text(encoding="utf-8"))
        for endpoint in data["endpoints"]:
            method = endpoint["method"].upper()
            api_path = endpoint["path"].rstrip("/") or "/"
            tokens = tuple(sorted(set(endpoint.get("tokens") or [])))
            key = (method, api_path)
            if key in merged:
                # Same endpoint may appear in the SDK catalogue and in a
                # manual/legacy catalogue with corrected token variants.
                old = merged[key]
                old["tokens"] = sorted(set(old["tokens"]) | set(tokens))
                if not old.get("summary") and endpoint.get("summary"):
                    old["summary"] = endpoint["summary"]
                if not old.get("doc_url") and endpoint.get("doc_url"):
                    old["doc_url"] = endpoint["doc_url"]
                if endpoint.get("name") and not old.get("name"):
                    old["name"] = endpoint["name"]
                if endpoint.get("multipart"):
                    old["multipart"] = True
                if endpoint.get("binary"):
                    old["binary"] = True
                continue
            merged[key] = {
                "domain": snake(endpoint["domain"]) or "root",
                "resource": snake(endpoint.get("resource", "")),
                "go_method": endpoint.get("go_method", "Call"),
                "method": method,
                "path": api_path,
                "tokens": list(tokens),
                "summary": clean_summary(endpoint.get("summary", "")),
                "doc_url": endpoint.get("doc_url", ""),
                "binary": bool(endpoint.get("binary")),
                "multipart": bool(endpoint.get("multipart")),
                "name": endpoint.get("name", ""),
            }
    return sorted(
        merged.values(),
        key=lambda e: (e["domain"], e["path"], e["method"], e["tokens"]),
    )


def base_name(endpoint: dict) -> str:
    domain = endpoint["domain"]
    segments = [s for s in endpoint["path"].split("/") if s]
    if segments and segments[0] == "open-apis":
        segments = segments[1:]
    if segments and segments[0] == domain:
        segments = segments[1:]
    filtered: list[str] = []
    for segment in segments:
        if re.fullmatch(r"v\d+", segment) or segment.startswith(":"):
            continue
        segment = snake(segment)
        if segment:
            filtered.append(segment)
    verb = snake(endpoint["go_method"] or "")
    if filtered and verb and (verb == filtered[-1] or filtered[-1].endswith("_" + verb)):
        filtered = filtered[:-1]
    parts = [domain] + filtered + ([verb] if verb else [])
    if len(parts) == 1:
        parts.append("call")
    return "_".join(parts)


def variants(tokens: list[str]) -> list[tuple[str, str]]:
    token_set = set(tokens)
    result: list[tuple[str, str]] = []
    if "Tenant" in token_set:
        result.append(("", "tenant"))
    if "User" in token_set:
        result.append(("_user", "user"))
    if "App" in token_set:
        result.append(("_app", "app"))
    if not result:
        result.append(("_public", "public"))
    return result


def sanitize_param(name: str) -> str:
    value = snake(name) or "path"
    if value in RUST_KEYWORDS or value in RESERVED_ARGS:
        value = "path_" + value
    return value


def parse_path(path: str) -> tuple[list[tuple[str, str]], str]:
    params: list[tuple[str, str]] = []
    used: set[str] = set()
    pieces: list[str] = []
    last = 0
    for match in re.finditer(r":([A-Za-z0-9_]+)", path):
        local = sanitize_param(match.group(1))
        base = local
        index = 1
        while local in used:
            index += 1
            local = f"{base}_{index}"
        used.add(local)
        params.append((local, match.group(1)))
        pieces.append(path[last : match.start()])
        pieces.append("{" + local + "}")
        last = match.end()
    pieces.append(path[last:])
    return params, "".join(pieces)


def render_method(endpoint: dict, name: str, suffix: str, identity: str) -> list[str]:
    method = endpoint["method"]
    # binary 优先：Go SDK 的 WithFileUpload() 也会出现在“下载原始 body”的
    # GET 端点上，此时不是 multipart 上传。
    binary = endpoint["binary"]
    multipart = endpoint.get("multipart", False) and not binary
    has_body = method != "GET" or multipart
    idempotent = method in ("GET", "DELETE")
    params, format_path = parse_path(endpoint["path"])

    args: list[str] = []
    if identity == "user":
        args.append("bearer_token: &str")
    elif identity == "app":
        args.append("app_access_token: &str")
    for local, _ in params:
        args.append(f"{local}: &str")
    args.append("query: &[(&str, &str)]")
    if multipart:
        args.append("content_type: &str")
        args.append("body: &[u8]")
    elif has_body:
        args.append("body: &serde_json::Value")

    return_type = "crate::transport::Response" if binary else "serde_json::Value"
    lines: list[str] = []

    summary = endpoint["summary"] or endpoint["go_method"]
    lines.append(f"    /// {summary}")
    lines.append("    ///")
    if endpoint["doc_url"]:
        lines.append(f"    /// 官方文档: <{endpoint['doc_url']}>")
    identity_doc = {
        "tenant": "需要 `tenant_access_token`（客户端自动获取）。",
        "user": "需要 `user_access_token`，由调用方传入 `bearer_token`。",
        "app": "需要 `app_access_token`，由调用方传入 `app_access_token`。",
        "public": "无鉴权；请求体需自带应用凭证。",
    }[identity]
    lines.append(f"    /// {identity_doc}")
    if multipart:
        lines.append(
            "    /// `multipart/form-data` 上传：调用方传入 `Content-Type` 与完整请求体。"
        )
    if binary:
        lines.append("    /// 二进制响应：返回原始 `Response`，由调用方读取 `bytes`。")

    body_arg_count = 2 if multipart else (1 if has_body else 0)
    argc = 1 + (1 if identity in ("user", "app") else 0) + len(params) + 1 + body_arg_count
    if argc > 7:
        lines.append("    #[allow(clippy::too_many_arguments)]")
    lines.append(
        f"    pub async fn {name}{suffix}(&self, {', '.join(args)}) -> Result<{return_type}> {{"
    )
    for local, original in params:
        lines.append(f'        validate_opaque_path_segment("{original}", {local})')
        lines.append(
            "            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;"
        )
    if params:
        lines.append(f'        let path = format!("{format_path}");')
    else:
        lines.append(f'        let path = "{endpoint["path"]}".to_string();')
    lines.append(
        "        let url = crate::api::join_url(self.core.endpoints().open.as_str(), &path);"
    )
    lines.append("        let query = crate::api::query_vec(query);")

    if multipart:
        call = {
            "tenant": "call_raw_json",
            "user": "call_raw_json_user",
            "app": "call_raw_json_user",
            "public": "call_raw_json_public",
        }[identity]
        prefix = ""
        if identity == "user":
            prefix = "bearer_token, "
        elif identity == "app":
            prefix = "app_access_token, "
        lines.append("        self.core")
        lines.append(
            f"            .{call}({prefix}Method::{METHOD_TITLE[method]}, url, query, content_type, body, 20, {str(idempotent).lower()})"
        )
        lines.append("            .await")
    elif binary:
        bearer = {
            "tenant": "None",
            "user": "Some(bearer_token.to_string())",
            "app": "Some(app_access_token.to_string())",
            "public": "None",
        }[identity]
        body_expr = "Some(body.clone())" if has_body else "None"
        send = "send_tenant" if identity == "tenant" else "send_raw"
        lines.append(f"        let response = self.core.{send}(")
        lines.append("            crate::transport::Request {")
        lines.append(f"                method: Method::{METHOD_TITLE[method]},")
        lines.append("                url,")
        lines.append("                query,")
        lines.append(f"                bearer_token: {bearer},")
        lines.append(f"                json_body: {body_expr},")
        lines.append("                raw_body: None,")
        lines.append("                binary_limit: Some(crate::api::MAX_BINARY_BYTES),")
        lines.append("                timeout_secs: Some(20),")
        lines.append(f"                idempotent: {str(idempotent).lower()},")
        lines.append("            },")
        lines.append("            &crate::transport::PULL_RETRY,")
        lines.append("        ).await?;")
        lines.append("        Ok(response)")
    else:
        body_expr = "Some(body.clone())" if has_body else "None"
        call = {
            "tenant": "call_json",
            "user": "call_json_user",
            "app": "call_json_user",
            "public": "call_json_public",
        }[identity]
        prefix = ""
        if identity == "user":
            prefix = "bearer_token, "
        elif identity == "app":
            prefix = "app_access_token, "
        lines.append("        self.core")
        lines.append(
            f"            .{call}({prefix}Method::{METHOD_TITLE[method]}, url, query, {body_expr}, 20, {str(idempotent).lower()})"
        )
        lines.append("            .await")
    lines.append("    }")
    return lines


def generate(endpoints: list[dict], out_dir: Path) -> dict:
    by_domain: dict[str, list[dict]] = defaultdict(list)
    for endpoint in endpoints:
        by_domain[endpoint["domain"]].append(endpoint)

    out_dir.mkdir(parents=True, exist_ok=True)
    stats: dict[str, int] = {}
    for domain in sorted(by_domain):
        used: set[str] = set()
        lines: list[str] = [
            f"//! `{domain}` 官方端点薄封装（自动生成）。",
            "//!",
            "//! 来源：官方 OpenAPI SDK/llms 端点目录。不要手工编辑；",
            "//! 重新生成见 `scripts/generate_openapi_endpoints.py`。",
            "",
            "use crate::api::ApiClient;",
            "use crate::error::Result;",
            "use crate::transport::Method;",
        ]
        has_params = any(":" in e["path"] for e in by_domain[domain])
        if has_params:
            lines.append("use crate::path::validate_opaque_path_segment;")
        lines.append("")
        lines.append("impl ApiClient {")
        method_count = 0
        for endpoint in sorted(
            by_domain[domain], key=lambda e: (e["path"], e["method"], e["tokens"])
        ):
            base = endpoint.get("name") or base_name(endpoint)
            suffixes = variants(endpoint["tokens"])
            candidate = base
            index = 0
            while any(candidate + suffix in used for suffix, _ in suffixes):
                index += 1
                candidate = f"{base}_{index}"
            for suffix, _ in suffixes:
                used.add(candidate + suffix)
            for suffix, identity in suffixes:
                lines.extend(render_method(endpoint, candidate, suffix, identity))
                lines.append("")
                method_count += 1
        lines.append("}")
        (out_dir / f"{domain}.rs").write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
        stats[domain] = method_count

    mod_lines = [
        "//! 官方 OpenAPI 全量端点薄封装。",
        "//!",
        "//! 由 `scripts/generate_openapi_endpoints.py` 从官方 SDK/llms 端点目录生成。",
        "//! 每个子模块对应一个官方域；方法名带域前缀，统一挂在 [`ApiClient`] 上。",
        "//! typed 核心链路仍使用 `bitable()` / `approval()` / `im()` 等客户端。",
        "",
        "use std::sync::Arc;",
        "",
        "use crate::client::ClientCore;",
        "",
        "/// 通用 OpenAPI 客户端。",
        "#[derive(Clone)]",
        "pub struct ApiClient {",
        "    // 只在启用生成域时被方法读取；默认空 feature 下保留装配面。",
        "    #[allow(dead_code)]",
        "    pub(crate) core: Arc<ClientCore>,",
        "}",
        "",
        "impl ApiClient {",
        "    pub(crate) fn new(core: Arc<ClientCore>) -> Self {",
        "        Self { core }",
        "    }",
        "}",
        "",
        "/// 二进制响应上限（64 MiB）。",
        "#[allow(dead_code)]",
        "pub(crate) const MAX_BINARY_BYTES: usize = 64 * 1024 * 1024;",
        "",
        "#[allow(dead_code)]",
        "pub(crate) fn query_vec(query: &[(&str, &str)]) -> Vec<(String, String)> {",
        "    query",
        "        .iter()",
        "        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))",
        "        .collect()",
        "}",
        "",
        "#[allow(dead_code)]",
        "pub(crate) fn join_url(base: &str, path: &str) -> String {",
        "    if path.starts_with(\"http://\") || path.starts_with(\"https://\") {",
        "        path.to_string()",
        "    } else {",
        "        format!(\"{base}{path}\")",
        "    }",
        "}",
        "",
    ]
    for domain in sorted(by_domain):
        mod_lines.append(f"/// `{domain}` 端点。")
        mod_lines.append(f'#[cfg(feature = "api-{domain}")]')
        mod_lines.append(f"pub mod {domain};")
        mod_lines.append("")
    (out_dir / "mod.rs").write_text("\n".join(mod_lines).rstrip() + "\n", encoding="utf-8")
    return stats


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--catalog", action="append", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    endpoints = load_endpoints([Path(p) for p in args.catalog])
    stats = generate(endpoints, Path(args.out))
    print(f"endpoints={len(endpoints)} domains={len(stats)} methods={sum(stats.values())}")
    for domain, count in sorted(stats.items(), key=lambda kv: (-kv[1], kv[0])):
        print(f"{domain}\t{count}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
