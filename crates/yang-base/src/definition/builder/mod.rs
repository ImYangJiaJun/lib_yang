//! App 定义构建、交叉校验与运行时 slot 预解析。

use std::collections::BTreeSet;

mod app;
mod catalog;
mod compile;
mod handle;
mod project;
mod registry;
mod validate;

pub use app::{AppBuilder, BuiltApp};
pub use catalog::DefinitionCatalog;
pub use handle::{ActionHandle, TypedActionHandle};
pub use registry::Registry;

/// 递归检测 JSON Schema 是否声明二进制文件字段（`format: "binary"`，
/// 即 `UploadedFile` 经 schemars 生成的形态；`$ref`/`anyOf`/`items` 递归覆盖）。
pub(crate) fn schema_contains_binary_field(schema: &serde_json::Value) -> bool {
    schema_subtree_contains_binary(schema, schema)
}

/// 递归检测 schema 子树是否声明二进制文件字段；本地 `$ref`（`#/definitions/...`）
/// 按 document 解析后继续深入，带循环保护（自引用类型不会死循环）。
pub(crate) fn schema_subtree_contains_binary(
    document: &serde_json::Value,
    subtree: &serde_json::Value,
) -> bool {
    fn inner(
        document: &serde_json::Value,
        subtree: &serde_json::Value,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        match subtree {
            serde_json::Value::Object(map) => {
                if map.get("format").and_then(serde_json::Value::as_str) == Some("binary") {
                    return true;
                }
                if let Some(name) = map
                    .get("$ref")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|reference| reference.strip_prefix("#/definitions/"))
                {
                    if visited.insert(name.to_string()) {
                        if let Some(target) = document
                            .get("definitions")
                            .and_then(|definitions| definitions.get(name))
                        {
                            if inner(document, target, visited) {
                                return true;
                            }
                        }
                    }
                }
                map.values().any(|value| inner(document, value, visited))
            }
            serde_json::Value::Array(items) => {
                items.iter().any(|item| inner(document, item, visited))
            }
            _ => false,
        }
    }
    inner(document, subtree, &mut BTreeSet::new())
}
