//! 受保护层校验：字段读/筛选/排序权限、WHERE 条件树递归校验
//!（含 QRY-1/QRY-2 上限与递归深度上限），以及默认可读投影。

use super::TableQuery;
use crate::error::BaseError;
use crate::table::WhereCondition;
use serde_json::Value;

impl TableQuery {
    /// 校验字段存在，并且当前角色具有读取权限。
    pub(super) fn validate_read_field(&self, field_name: &str) -> Result<(), BaseError> {
        let field_config = self.table_config.get_field(field_name).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field_name.to_string())
        })?;

        if !field_config.permissions.can_read(&self.user_roles_set) {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field_name.to_string(),
                "用户无读取权限".to_string(),
            ));
        }

        Ok(())
    }

    #[cfg(feature = "mysql")]
    pub(super) fn default_read_fields(&self) -> Result<Vec<&str>, BaseError> {
        let mut fields: Vec<&str> = self
            .table_config
            .fields
            .iter()
            .filter_map(|(name, field)| {
                (!field.hidden && field.permissions.can_read(&self.user_roles_set))
                    .then_some(name.as_str())
            })
            .collect();
        fields.sort_unstable();
        if fields.is_empty() {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                "*".to_string(),
                "当前角色没有可读字段".to_string(),
            ));
        }
        Ok(fields)
    }

    #[cfg(feature = "mysql")]
    pub(crate) fn ensure_readable_projection(&self) -> Result<(), BaseError> {
        self.default_read_fields().map(|_| ())
    }

    /// 添加排序规则 (ORDER BY field direction)
    ///
    /// 添加排序规则，并验证：
    /// 1. 字段是否存在于表配置中
    /// 2. 用户是否有字段的排序权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    /// - `order`：排序方向 (Asc 或 Desc)
    ///
    /// # 返回值
    ///
    /// - `Ok(Self)`：验证通过，返回 self 支持链式调用
    /// - `Err(BaseError)`：验证失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无排序权限
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::SortOrder;
    ///
    /// let query = query.order_by("created_at", SortOrder::Desc)?;
    /// ```
    pub(super) fn validate_order_field(&self, field: &str) -> Result<(), BaseError> {
        // 1. 检查字段是否存在
        let field_config = self.table_config.get_field(field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
        })?;

        // 2. 字段级排序开关：标记为不可排序的字段直接拒绝（先于角色权限，
        //    确保 `.sortable(false)` 是硬约束而非可被空角色列表绕过的软提示）
        if !field_config.sortable {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "字段不允许排序".to_string(),
            ));
        }

        // 3. 检查用户是否有排序权限
        if !field_config.permissions.can_sort(&self.user_roles_set) {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "用户无排序权限".to_string(),
            ));
        }

        Ok(())
    }

    /// 验证筛选字段的权限
    ///
    /// 内部辅助方法，用于验证字段是否存在以及用户是否有筛选权限
    ///
    /// # 参数
    ///
    /// - `field`：字段名
    ///
    /// # 返回值
    ///
    /// - `Ok(())`：验证通过
    /// - `Err(BaseError)`：验证失败
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无筛选权限
    fn validate_filter_field(&self, field: &str) -> Result<(), BaseError> {
        // 1. 检查字段是否存在
        let field_config = self.table_config.get_field(field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
        })?;

        // 2. 字段级筛选开关：标记为不可筛选的字段直接拒绝（先于角色权限，
        //    确保 `.filterable(false)` 是硬约束而非可被空角色列表绕过的软提示）
        if !field_config.filterable {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "字段不允许筛选".to_string(),
            ));
        }

        // 3. 检查用户是否有筛选权限
        if !field_config.permissions.can_filter(&self.user_roles_set) {
            return Err(BaseError::FieldPermissionDenied(
                self.table_config.table_name.clone(),
                field.to_string(),
                "用户无筛选权限".to_string(),
            ));
        }

        Ok(())
    }

    /// 通过同一校验边界追加任意 WHERE 条件，避免各链式入口出现校验差异。
    pub(super) fn push_where_condition(
        mut self,
        condition: WhereCondition,
    ) -> Result<Self, BaseError> {
        self.validate_condition_tree(&condition, 0)?;
        self.query_params.where_conditions.push(condition);
        Ok(self)
    }

    /// 校验叶子条件的操作符与字段类型兼容，并验证每一个参与比较的值。
    fn validate_condition_values(
        &self,
        condition: &WhereCondition,
        field: &str,
    ) -> Result<(), BaseError> {
        let field_config = self.table_config.get_field(field).ok_or_else(|| {
            BaseError::FieldNotFound(self.table_config.table_name.clone(), field.to_string())
        })?;
        let field_type = &field_config.field_type;

        let reject_operator = |operator: &str| {
            BaseError::ParamInvalid(
                field.to_string(),
                format!("字段类型 {field_type:?} 不支持 {operator} 条件"),
            )
        };
        let validate = |value: &Value| field_type.validate(field, value);
        let is_orderable = matches!(
            field_type,
            crate::table::FieldType::String { .. }
                | crate::table::FieldType::Integer
                | crate::table::FieldType::BigInt
                | crate::table::FieldType::Float
                | crate::table::FieldType::Double
                | crate::table::FieldType::Decimal { .. }
                | crate::table::FieldType::Date
                | crate::table::FieldType::DateTime
                | crate::table::FieldType::Timestamp
                | crate::table::FieldType::Text
                | crate::table::FieldType::Enum { .. }
        );
        let is_textual = matches!(
            field_type,
            crate::table::FieldType::String { .. }
                | crate::table::FieldType::Text
                | crate::table::FieldType::Enum { .. }
        );

        match condition {
            WhereCondition::Eq { value, .. } | WhereCondition::Ne { value, .. } => {
                // NULL 比较由渲染器规范化为 IS NULL / IS NOT NULL，不作为字段值校验。
                if value.is_null() {
                    Ok(())
                } else {
                    validate(value)
                }
            }
            WhereCondition::In { values, .. } | WhereCondition::NotIn { values, .. } => {
                values.iter().try_for_each(validate)
            }
            WhereCondition::Like { .. } => {
                if is_textual {
                    Ok(())
                } else {
                    Err(reject_operator("LIKE"))
                }
            }
            WhereCondition::Gt { value, .. }
            | WhereCondition::Gte { value, .. }
            | WhereCondition::Lt { value, .. }
            | WhereCondition::Lte { value, .. } => {
                if !is_orderable {
                    return Err(reject_operator("范围比较"));
                }
                validate(value)
            }
            WhereCondition::Between { lo, hi, .. } => {
                if !is_orderable {
                    return Err(reject_operator("BETWEEN"));
                }
                validate(lo)?;
                validate(hi)
            }
            WhereCondition::IsNull { .. } | WhereCondition::IsNotNull { .. } => Ok(()),
            WhereCondition::And { .. } | WhereCondition::Or { .. } => Err(BaseError::ParamInvalid(
                "condition".to_string(),
                "逻辑组不能作为叶子条件校验".to_string(),
            )),
        }
    }

    /// 嵌套布尔条件的最大递归深度，防止深层嵌套（或恶意构造）爆栈。
    ///
    /// 校验期（`validate_condition_tree`）与渲染期（`render_condition`）共用同一上限。
    pub(super) const MAX_WHERE_DEPTH: usize = 32;

    /// LIKE pattern 最大字节长度（QRY-1 防 DoS）。
    ///
    /// 超长 pattern 会导致 MySQL 索引失效、全表扫描放大；`where_like` /
    /// `where_contains` / 校验期 / 渲染期统一拦截。
    pub(super) const MAX_LIKE_PATTERN_LEN: usize = 128;

    /// IN / NOT IN 列表最大元素数（QRY-2 防 DoS）。
    ///
    /// 过长的 IN 列表会导致 MySQL 优化器退化、解析/绑定开销放大；`where_in` /
    /// `where_not_in` / 校验期 / 渲染期统一拦截。
    pub(super) const MAX_IN_LIST_SIZE: usize = 500;

    /// 递归校验一棵 WHERE 条件树的字段与筛选权限。
    ///
    /// 叶子条件校验其字段存在且当前角色可筛选；逻辑组（`And`/`Or`）递归下钻校验
    /// 每个子条件。深度超过 [`Self::MAX_WHERE_DEPTH`] 返回 `ParamInvalid` 而非
    /// panic，与渲染期保持一致的防爆栈上限。
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：某叶子字段不存在
    /// - `BaseError::FieldPermissionDenied`：某叶子字段无筛选权限
    /// - `BaseError::ParamInvalid`：嵌套层数超限
    pub(super) fn validate_condition_tree(
        &self,
        condition: &WhereCondition,
        depth: usize,
    ) -> Result<(), BaseError> {
        if depth > Self::MAX_WHERE_DEPTH {
            return Err(BaseError::ParamInvalid(
                "where".to_string(),
                format!("嵌套布尔条件层数超过上限 {}", Self::MAX_WHERE_DEPTH),
            ));
        }

        match condition {
            WhereCondition::And { conditions } | WhereCondition::Or { conditions } => {
                // 空布尔组拒绝：空 And 渲染为 `1=1`、空 Or 渲染为 `1=0`，前者会使
                // `where_conditions` 非空从而绕过 UPDATE/DELETE 的全表写守卫，生成
                // `WHERE (1=1)` 全表改写。在校验期直接拒绝空组，杜绝该绕过路径。
                if conditions.is_empty() {
                    return Err(BaseError::ParamInvalid(
                        "where".to_string(),
                        "AND/OR 布尔组不能为空".to_string(),
                    ));
                }
                for child in conditions {
                    self.validate_condition_tree(child, depth + 1)?;
                }
                Ok(())
            }
            // 叶子：必有字段，校验存在性与筛选权限；同时校验 LIKE/IN 上限（QRY-1/QRY-2）
            leaf => {
                let field = leaf.field().ok_or_else(|| {
                    BaseError::ParamInvalid(
                        "condition".to_string(),
                        "条件节点缺少字段名".to_string(),
                    )
                })?;
                self.validate_filter_field(field)?;
                // LIKE pattern 长度上限（QRY-1）
                if let WhereCondition::Like { pattern, .. } = leaf {
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
                }
                // IN / NOT IN 列表元素数上限（QRY-2）
                if let WhereCondition::In { values, .. } | WhereCondition::NotIn { values, .. } =
                    leaf
                {
                    if values.is_empty() {
                        return Err(BaseError::ParamInvalid(
                            "values".to_string(),
                            "IN/NOT IN 列表不能为空".to_string(),
                        ));
                    }

                    if values.len() > Self::MAX_IN_LIST_SIZE {
                        return Err(BaseError::ParamInvalid(
                            "values".to_string(),
                            format!(
                                "IN/NOT IN 列表元素数 {} 超过上限 {}",
                                values.len(),
                                Self::MAX_IN_LIST_SIZE
                            ),
                        ));
                    }
                }
                self.validate_condition_values(leaf, field)
            }
        }
    }
}
