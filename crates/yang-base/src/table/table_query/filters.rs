//! 链式查询构建入口：字段选择、WHERE 条件、布尔组、排序、分页、
//! 搜索、软删除/租户范围开关，以及查询参数访问器。

use super::{TableQuery, MAX_TABLE_QUERY_PAGE_SIZE};
use crate::error::BaseError;
use crate::table::{QueryParams, SortOrder, TableConfig, WhereCondition};
use serde_json::Value;
use std::sync::Arc;

impl TableQuery {
    /// 选择要查询的字段。
    ///
    /// 会校验字段存在，并校验当前用户角色具备这些字段的读取权限。
    pub fn select_fields(mut self, fields: &[&str]) -> Result<Self, BaseError> {
        if fields.is_empty() {
            return Err(BaseError::ParamInvalid(
                "fields".to_string(),
                "查询字段列表不能为空".to_string(),
            ));
        }

        // 验证每个字段
        for field_name in fields {
            self.validate_read_field(field_name)?;
        }

        // 设置字段列表
        self.query_params.fields = Some(fields.iter().map(|s| s.to_string()).collect());

        Ok(self)
    }

    /// 添加等于条件 (WHERE field = value)
    ///
    /// 添加 WHERE 等于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use serde_json::json;
    ///
    /// let query = query.where_eq("status", json!("active"))?;
    /// ```
    pub fn where_eq(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Eq {
            field: field.to_string(),
            value,
        })
    }

    /// 添加包含条件 (WHERE field IN (values))
    ///
    /// 添加 WHERE IN 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `values`：值列表
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use serde_json::json;
    ///
    /// let query = query.where_in("status", vec![json!(1), json!(2), json!(3)])?;
    /// ```
    pub fn where_in(self, field: &str, values: Vec<Value>) -> Result<Self, BaseError> {
        if values.is_empty() {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                "IN 列表不能为空".to_string(),
            ));
        }

        // QRY-2: IN 列表元素数上限
        if values.len() > Self::MAX_IN_LIST_SIZE {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                format!(
                    "IN 列表元素数 {} 超过上限 {}",
                    values.len(),
                    Self::MAX_IN_LIST_SIZE
                ),
            ));
        }

        self.push_where_condition(WhereCondition::In {
            field: field.to_string(),
            values,
        })
    }

    /// 添加模糊匹配条件 (WHERE field LIKE pattern)
    ///
    /// 添加 WHERE LIKE 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `pattern`：匹配模式，支持 % 和 _ 通配符
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let query = query.where_like("name", "%alice%")?;
    /// ```
    pub fn where_like(self, field: &str, pattern: String) -> Result<Self, BaseError> {
        // QRY-1: LIKE pattern 长度上限
        if pattern.len() > Self::MAX_LIKE_PATTERN_LEN {
            return Err(BaseError::ParamInvalid(
                "pattern".to_string(),
                format!(
                    "LIKE pattern 长度 {} 超过上限 {}",
                    pattern.len(),
                    Self::MAX_LIKE_PATTERN_LEN
                ),
            ));
        }

        self.push_where_condition(WhereCondition::Like {
            field: field.to_string(),
            pattern,
        })
    }

    /// 添加模糊匹配条件 (WHERE field LIKE '%keyword%')
    ///
    /// 便捷方法：自动将 `keyword` 用 `%` 包裹，并转义其中的 `%` 和 `_` 通配符，
    /// 避免用户输入中的通配符被解释为 SQL LIKE 语法。
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `keyword`：搜索关键词（无需手动加 `%`）
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败（字段不存在 / 无权限 / 转义后超长）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // WHERE name LIKE '%alice%'，且 "%" / "_" 被转义
    /// let query = query.where_contains("name", "alice")?;
    /// ```
    pub fn where_contains(mut self, field: &str, keyword: &str) -> Result<Self, BaseError> {
        if keyword.trim().is_empty() {
            return Err(BaseError::ParamInvalid(
                "keyword".to_string(),
                "搜索关键词不能为空".to_string(),
            ));
        }

        // 转义 LIKE 通配符：% → \%，_ → \_
        let escaped = keyword
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{}%", escaped);

        // 复用 where_like 的校验（含 pattern 长度上限 + 字段权限）
        self = self.where_like(field, pattern)?;
        Ok(self)
    }

    /// 添加不等于条件 (WHERE field <> value)
    ///
    /// 添加 WHERE 不等于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_ne(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Ne {
            field: field.to_string(),
            value,
        })
    }

    /// 添加小于条件 (WHERE field < value)
    ///
    /// 添加 WHERE 小于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_lt(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Lt {
            field: field.to_string(),
            value,
        })
    }

    /// 添加小于等于条件 (WHERE field <= value)
    ///
    /// 添加 WHERE 小于等于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_lte(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Lte {
            field: field.to_string(),
            value,
        })
    }

    /// 添加大于条件 (WHERE field > value)
    ///
    /// 添加 WHERE 大于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_gt(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Gt {
            field: field.to_string(),
            value,
        })
    }

    /// 添加大于等于条件 (WHERE field >= value)
    ///
    /// 添加 WHERE 大于等于条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `value`：比较值
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_gte(self, field: &str, value: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Gte {
            field: field.to_string(),
            value,
        })
    }

    /// 添加区间条件 (WHERE field BETWEEN lo AND hi)
    ///
    /// 添加 WHERE BETWEEN 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// 当 `lo > hi` 时 BETWEEN 返回空集（MySQL 标准行为，框架不做特殊处理）。
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `lo`：区间下界（包含）
    /// - `hi`：区间上界（包含）
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_between(self, field: &str, lo: Value, hi: Value) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::Between {
            field: field.to_string(),
            lo,
            hi,
        })
    }

    /// 添加空值判断 (WHERE field IS NULL)
    ///
    /// 添加 WHERE IS NULL 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_null(self, field: &str) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::IsNull {
            field: field.to_string(),
        })
    }

    /// 添加非空值判断 (WHERE field IS NOT NULL)
    ///
    /// 添加 WHERE IS NOT NULL 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_not_null(self, field: &str) -> Result<Self, BaseError> {
        self.push_where_condition(WhereCondition::IsNotNull {
            field: field.to_string(),
        })
    }

    /// 添加不在列表条件 (WHERE field NOT IN (values))
    ///
    /// 添加 WHERE NOT IN 条件，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `values`：排除值列表
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    pub fn where_not_in(self, field: &str, values: Vec<Value>) -> Result<Self, BaseError> {
        if values.is_empty() {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                "NOT IN 列表不能为空".to_string(),
            ));
        }

        // QRY-2: NOT IN 列表元素数上限
        if values.len() > Self::MAX_IN_LIST_SIZE {
            return Err(BaseError::ParamInvalid(
                "values".to_string(),
                format!(
                    "NOT IN 列表元素数 {} 超过上限 {}",
                    values.len(),
                    Self::MAX_IN_LIST_SIZE
                ),
            ));
        }

        self.push_where_condition(WhereCondition::NotIn {
            field: field.to_string(),
            values,
        })
    }

    /// 添加排序规则。
    ///
    /// 会校验字段存在、字段允许排序，以及当前用户角色具备排序权限。
    pub fn order_by(mut self, field: &str, order: SortOrder) -> Result<Self, BaseError> {
        self.validate_order_field(field)?;

        // 添加排序规则
        self.query_params.order_by.push((field.to_string(), order));

        Ok(self)
    }

    /// 设置分页参数 (LIMIT and OFFSET)
    ///
    /// 设置查询的分页参数
    ///
    /// # 参数
    ///
    /// - `page`：当前页码，从 1 开始
    /// - `page_size`：每页大小
    ///
    /// # 返回值
    ///
    /// 返回 self 支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let query = query.page(1, 20)?;
    /// ```
    pub fn page(mut self, page: usize, page_size: usize) -> Result<Self, BaseError> {
        if page == 0 || page_size == 0 {
            return Err(BaseError::ParamInvalid(
                "page".to_string(),
                "页码与每页大小必须从 1 开始且大于 0".to_string(),
            ));
        }
        if page_size > MAX_TABLE_QUERY_PAGE_SIZE {
            return Err(BaseError::ParamInvalid(
                "page_size".to_string(),
                format!("每页大小不能超过 {}", MAX_TABLE_QUERY_PAGE_SIZE),
            ));
        }
        self.query_params.page = Some(page);
        self.query_params.page_size = Some(page_size);
        Ok(self)
    }

    /// 为服务端有界预取设置从首行开始的硬上限。
    ///
    /// 该入口只供 crate 内已经持有可信上限的算法使用（例如树查询的
    /// `max_nodes + 1` 截断检测），不接受终端用户分页参数，因此不套用公开分页的
    /// 100 行产品限制。
    #[cfg(feature = "mysql")]
    pub(crate) fn prefetch_limit(mut self, limit: usize) -> Result<Self, BaseError> {
        if limit == 0 {
            return Err(BaseError::ParamInvalid(
                "limit".to_string(),
                "预取上限必须大于 0".to_string(),
            ));
        }
        self.query_params.page = Some(1);
        self.query_params.page_size = Some(limit);
        Ok(self)
    }

    /// 对声明了 searchable 且当前角色可读的文本字段应用一次 OR LIKE 搜索。
    ///
    /// 关键词搜索只认独立的 searchable 位（[`crate::table::Field::searchable`]），与
    /// 结构化 where 的 filterable 校验（`validate_filter_field`）互不开放。搜索字段
    /// 来自表定义迭代（必然存在）且已逐一验证 `is_text` / `searchable` / 可读 /
    /// 非 hidden，因此这里自行构造 OR 组、不经过 `where_or` 的 filterable 门槛；
    /// 但 LIKE pattern 长度上限（QRY-1）仍在本地强制执行。
    pub fn search(mut self, keyword: Option<&str>) -> Result<Self, BaseError> {
        let Some(keyword) = keyword.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(self);
        };
        let mut fields = self
            .table_config
            .fields
            .iter()
            .filter(|(_, field)| {
                field.field_type.is_text()
                    && field.searchable
                    && field.permissions.can_read(&self.user_roles_set)
                    && !field.hidden
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        fields.sort();
        if fields.is_empty() {
            return Err(BaseError::PermissionDenied(format!(
                "表 {} 没有当前角色可搜索的文本字段",
                self.table_config.table_name
            )));
        }
        let pattern = format!("%{keyword}%");
        if pattern.len() > Self::MAX_LIKE_PATTERN_LEN {
            return Err(BaseError::ParamInvalid(
                "pattern".to_string(),
                format!(
                    "LIKE pattern 长度 {} 超过上限 {}",
                    pattern.len(),
                    Self::MAX_LIKE_PATTERN_LEN
                ),
            ));
        }
        let group = WhereCondition::Or {
            conditions: fields
                .into_iter()
                .map(|field| WhereCondition::Like {
                    field,
                    pattern: pattern.clone(),
                })
                .collect(),
        };
        self.query_params.where_conditions.push(group);
        Ok(self)
    }

    /// 在读取路径包含软删除记录
    ///
    /// 默认情况下，配置了 `soft_delete_field` 的表会在 select/count/paginate
    /// 时自动追加 `软删字段 IS NULL` 过滤，隐藏已软删行。调用本方法后，本次查询
    /// 读取全量数据（含已软删行）。
    ///
    /// # 返回值
    ///
    /// 返回 self 支持链式调用
    pub fn with_trashed(mut self) -> Self {
        self.include_trashed = true;
        self
    }

    /// 注入强制租户范围。该条件绕过业务筛选权限，但字段必须是定义中的 tenant key。
    pub(crate) fn scope_tenant(mut self, field: &str, value: Value) -> Result<Self, BaseError> {
        let config = self.table_config.get_field(field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
        })?;
        if !config.tenant_key {
            return Err(BaseError::ConfigError(format!(
                "字段 {}.{} 未声明为 tenant_key",
                self.table_config.table_name, field
            )));
        }
        config.field_type.validate(field, &value)?;
        self.query_params.where_conditions.push(WhereCondition::Eq {
            field: field.to_string(),
            value: value.clone(),
        });
        self.tenant_scope = Some((field.to_string(), value));
        Ok(self)
    }

    /// 注入受信的主键等值条件。
    ///
    /// 内置 get/put/del 的主键定位是 Action 自有寻址机制，不是调用方可选的结构化
    /// 筛选，因此与 [`Self::scope_tenant`] 一样绕过 filterable 业务筛选权限；
    /// 值仍按主键字段类型校验（null 交由渲染器规范化为 IS NULL，匹配不到记录）。
    #[cfg(feature = "mysql")]
    pub fn where_primary_key_eq(mut self, value: Value) -> Result<Self, BaseError> {
        let field = self.table_config.primary_key.clone();
        let config = self.table_config.get_field(&field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.clone())
        })?;
        if !value.is_null() {
            config.field_type.validate(&field, &value)?;
        }
        self.query_params
            .where_conditions
            .push(WhereCondition::Eq { field, value });
        Ok(self)
    }

    /// 添加一个 OR 逻辑组 (WHERE ... AND (c1 OR c2 OR ...))
    ///
    /// 组内每个子条件递归校验字段存在性与筛选权限；通过后整组以 `Or` 节点追加到
    /// 顶层条件列表，与既有条件以隐式 AND 连接。空组等价于恒假（`1=0`）。
    ///
    /// 子条件可由 [`WhereCondition`] 直接构造，亦可嵌套 `And`/`Or` 组（深度上限
    /// `MAX_WHERE_DEPTH`）。
    ///
    /// # 参数
    ///
    /// - `conditions`：OR 组的子条件列表
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：校验通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：字段不存在 / 无筛选权限 / 嵌套超限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::WhereCondition;
    /// use serde_json::json;
    ///
    /// // WHERE status = 'active' AND (age >= 18 OR vip = true)
    /// let query = query
    ///     .where_eq("status", json!("active"))?
    ///     .where_or(vec![
    ///         WhereCondition::Gte { field: "age".into(), value: json!(18) },
    ///         WhereCondition::Eq { field: "vip".into(), value: json!(true) },
    ///     ])?;
    /// ```
    pub fn where_or(mut self, conditions: Vec<WhereCondition>) -> Result<Self, BaseError> {
        let group = WhereCondition::Or { conditions };
        // 递归校验整棵子树（含嵌套组）
        self.validate_condition_tree(&group, 0)?;
        self.query_params.where_conditions.push(group);
        Ok(self)
    }

    /// 添加一个 AND 逻辑组 (WHERE ... AND (c1 AND c2 AND ...))
    ///
    /// 语义同 [`TableQuery::where_or`]，但组内子条件以 AND 连接。主要用于在 OR 组
    /// 内部嵌套 AND 子组；顶层多个条件本就隐式 AND，单独使用通常无必要。空组等价
    /// 于恒真（`1=1`）。
    ///
    /// # 参数
    ///
    /// - `conditions`：AND 组的子条件列表
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：校验通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：字段不存在 / 无筛选权限 / 嵌套超限
    pub fn where_and(mut self, conditions: Vec<WhereCondition>) -> Result<Self, BaseError> {
        let group = WhereCondition::And { conditions };
        self.validate_condition_tree(&group, 0)?;
        self.query_params.where_conditions.push(group);
        Ok(self)
    }

    /// 追加一棵任意 WHERE 条件（叶子或 `And`/`Or` 组），递归校验后并入顶层条件。
    ///
    /// 这是以 [`WhereCondition`] 表示的类型化布尔树桥接到受保护层的统一入口：
    /// 整棵树先经 `validate_condition_tree` 递归校验字段存在性、筛选权限与
    /// 嵌套深度，通过后作为单个条件追加（与既有条件隐式 AND 连接）。
    ///
    /// # 参数
    ///
    /// - `condition`：任意 `WhereCondition`（叶子或逻辑组）
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：校验通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：字段不存在 / 无筛选权限 / 嵌套超限
    pub fn where_tree(mut self, condition: WhereCondition) -> Result<Self, BaseError> {
        self.validate_condition_tree(&condition, 0)?;
        self.query_params.where_conditions.push(condition);
        Ok(self)
    }

    /// 获取查询参数的引用
    ///
    /// 用于测试或调试，获取当前构建的查询参数
    ///
    /// # 返回值
    ///
    /// 返回查询参数的引用
    #[allow(dead_code)]
    pub fn get_query_params(&self) -> &QueryParams {
        &self.query_params
    }

    /// 应用通用列表参数，并复用本类型现有的字段/筛选/排序/分页权限校验。
    pub fn apply_params(mut self, mut params: QueryParams) -> Result<Self, BaseError> {
        params.normalize();
        if let Some(fields) = params.fields {
            let names = fields.iter().map(String::as_str).collect::<Vec<_>>();
            self = self.select_fields(&names)?;
        }
        for condition in params.where_conditions {
            self = self.where_tree(condition)?;
        }
        for (field, order) in params.order_by {
            self = self.order_by(&field, order)?;
        }
        if params.page.is_some() || params.page_size.is_some() {
            self = self.page(
                params.page.unwrap_or(1),
                params
                    .page_size
                    .unwrap_or(crate::table::query_params::DEFAULT_QUERY_PAGE_SIZE),
            )?;
        }
        Ok(self)
    }

    /// 获取表配置的引用
    ///
    /// 用于测试或调试，获取表配置
    ///
    /// # 返回值
    ///
    /// 返回表配置的引用
    #[allow(dead_code)]
    pub(crate) fn get_table_config(&self) -> &Arc<TableConfig> {
        &self.table_config
    }

    /// 获取用户角色列表的引用
    ///
    /// 用于测试或调试，获取用户角色列表
    ///
    /// # 返回值
    ///
    /// 返回用户角色列表的引用
    #[allow(dead_code)]
    pub fn get_user_roles(&self) -> &[String] {
        &self.user_roles
    }
}
