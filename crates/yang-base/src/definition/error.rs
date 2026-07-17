//! 定义构建错误。

use thiserror::Error;

/// App 定义在冻结前发现的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum BuildError {
    /// 名称不符合对应命名规则。
    #[error("{kind} 名称非法: {name}")]
    InvalidName {
        /// 名称种类。
        kind: &'static str,
        /// 非法名称。
        name: String,
    },
    /// 同一命名空间出现重复名称。
    #[error("{kind} 名称重复: {name}")]
    DuplicateName {
        /// 名称种类。
        kind: &'static str,
        /// 重复名称。
        name: String,
    },
    /// 定义引用的目标不存在。
    #[error("{kind} 引用无效: {reference}")]
    InvalidReference {
        /// 引用种类。
        kind: &'static str,
        /// 无法解析的引用。
        reference: String,
    },
    /// HTTP method 与 path 的匹配集合发生冲突。
    #[error("route 冲突: {method} {path}{detail}")]
    RouteConflict {
        /// HTTP method。
        method: String,
        /// 路由模板。
        path: String,
        /// 底层模板匹配器提供的可选细节。
        detail: String,
    },
    /// 路由声明本身非法。
    #[error("route 非法: {method} {path}: {reason}")]
    InvalidRoute {
        /// HTTP method。
        method: String,
        /// 路由模板。
        path: String,
        /// 失败原因。
        reason: String,
    },
    /// 字段定义内部存在矛盾或无效配置。
    #[error("字段定义非法 [{table}.{field}]: {reason}")]
    InvalidFieldDefinition {
        /// 表名。
        table: String,
        /// 字段名。
        field: String,
        /// 失败原因。
        reason: String,
    },
    /// Addon 声明的依赖没有注册。
    #[error("Addon 依赖缺失 [{addon}]: {dependency}")]
    DependencyMissing {
        /// 声明依赖的 Addon。
        addon: String,
        /// 缺失的 Addon。
        dependency: String,
    },
}
