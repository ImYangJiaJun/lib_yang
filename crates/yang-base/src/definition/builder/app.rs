//! BuiltApp 冻结定义与 AppBuilder 构建编排。

use crate::action::{ActionContext, ApiResponse, Request};
use crate::definition::{AddonSpec, BuildError};
use crate::error::BaseError;
use crate::table::TableDefinition;
use crate::tools::Tools;
use std::fmt;
use std::sync::Arc;

use super::catalog::DefinitionCatalog;
use super::compile::{
    build_registry, compile_runtime_modules, compile_runtime_table_views,
    compile_table_definitions, compile_views,
};
use super::handle::ActionHandle;
use super::registry::Registry;
use super::validate::{
    collect_actions, collect_fields, collect_views, resolve_param_fields, sort_definitions,
    validate_dependencies, validate_middleware_order, validate_module_contents,
    validate_module_ownership, validate_references, validate_routes, validate_unique_addons,
    validate_unique_modules,
};

/// 构建完成且运行期不可变的 App 定义。
#[derive(Clone)]
pub struct BuiltApp {
    catalog: DefinitionCatalog,
    registry: Arc<Registry>,
    tools: Arc<Tools>,
    table_definitions: Vec<TableDefinition>,
    compiled_views: Vec<crate::definition::CompiledTableView>,
}

impl fmt::Debug for BuiltApp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BuiltApp")
            .field("catalog", &self.catalog)
            .field("registry", &self.registry)
            .field("tools", &self.tools)
            .field("table_definition_count", &self.table_definitions.len())
            .field("compiled_view_count", &self.compiled_views.len())
            .finish()
    }
}

impl BuiltApp {
    /// 返回只读定义 Catalog。
    pub fn catalog(&self) -> &DefinitionCatalog {
        &self.catalog
    }

    /// 返回预解析 Registry。
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// 返回当前应用实例显式拥有的冻结资源。
    pub fn tools(&self) -> &Tools {
        &self.tools
    }

    /// 返回从 Catalog 唯一字段事实预编译的数据库 schema 定义。
    pub fn table_definitions(&self) -> &[TableDefinition] {
        &self.table_definitions
    }

    /// 返回启动期解析完成的默认/显式 Table Views。
    pub fn compiled_views(&self) -> &[crate::definition::CompiledTableView] {
        &self.compiled_views
    }

    /// 按当前请求的认证身份投影有权访问的版本化 UI 目录。
    ///
    /// Action 和 View 均复用 dispatch 的构建期冻结授权策略；TableView 列同时按
    /// 字段读取角色过滤，secret 字段始终 fail-closed。租户数据范围仍由实际
    /// TableQuery 独立强制执行，前端目录不能替代服务端数据隔离。
    pub fn ui_catalog(
        &self,
        context: &ActionContext,
    ) -> Result<crate::definition::UiCatalog, BaseError> {
        self.registry.ui_catalog(context)
    }

    /// 为一次请求创建绑定当前应用资源的 ActionContext。
    pub fn context(&self, request: Request) -> ActionContext {
        ActionContext::new(request, Arc::clone(&self.tools))
            .with_registry(Arc::clone(&self.registry))
    }

    /// 通过启动期解析的 handle 执行请求。
    pub async fn dispatch(
        &self,
        handle: ActionHandle,
        request: Request,
    ) -> Result<ApiResponse, BaseError> {
        self.registry.dispatch(handle, self.context(request)).await
    }

    /// 执行已由传输层补全元数据的上下文。
    pub async fn dispatch_context(
        &self,
        handle: ActionHandle,
        context: ActionContext,
    ) -> Result<ApiResponse, BaseError> {
        self.registry.dispatch(handle, context).await
    }
}

/// 仅在启动期可变的 App 定义构建器。
#[derive(Debug, Clone, Default)]
#[must_use = "AppBuilder 必须调用 build() 才会执行交叉校验并冻结定义"]
pub struct AppBuilder {
    addons: Vec<AddonSpec>,
}

impl AppBuilder {
    /// 创建空构建器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个原生 Addon 定义。
    pub fn addon(mut self, addon: impl Into<AddonSpec>) -> Self {
        self.addons.push(addon.into());
        self
    }

    /// 校验全部定义、解析引用并冻结 Catalog/Registry。
    pub fn build(mut self, tools: impl Into<Arc<Tools>>) -> Result<BuiltApp, BuildError> {
        validate_unique_addons(&self.addons)?;
        validate_dependencies(&self.addons)?;
        validate_module_ownership(&self.addons)?;
        validate_unique_modules(&self.addons)?;
        validate_module_contents(&self.addons)?;
        validate_middleware_order(&self.addons)?;
        resolve_param_fields(&mut self.addons)?;

        let fields = collect_fields(&self.addons)?;
        let actions = collect_actions(&self.addons)?;
        let views = collect_views(&self.addons)?;
        validate_references(&self.addons, &fields, &actions, &views)?;
        validate_routes(&self.addons)?;

        sort_definitions(&mut self.addons);
        let mut registry = build_registry(&self.addons)?;
        let compiled_views = compile_views(&self.addons, &registry)?;
        registry.table_views = compile_runtime_table_views(&self.addons, &registry)?;
        registry.modules = compile_runtime_modules(&self.addons, &registry)?;
        for runtime in &registry.handlers {
            runtime.handler.bind_registry(&registry).map_err(|error| {
                BuildError::InvalidReference {
                    kind: "ActionLink",
                    reference: error.to_string(),
                }
            })?;
        }
        let registry = Arc::new(registry);
        let table_definitions = compile_table_definitions(&self.addons)?;
        for module in self.addons.iter_mut().flat_map(|addon| &mut addon.modules) {
            module.clear_handlers();
        }
        Ok(BuiltApp {
            catalog: DefinitionCatalog {
                addons: self.addons,
            },
            registry,
            tools: tools.into(),
            table_definitions,
            compiled_views,
        })
    }
}
