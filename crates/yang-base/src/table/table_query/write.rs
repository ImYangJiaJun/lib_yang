//! 写入执行（`mysql` feature）：insert/update/delete 及事务内变体，
//! 含插入数据准备（默认值/时间戳/租户注入）与更新数据校验。

#![cfg(feature = "mysql")]

use super::TableQuery;
use crate::error::BaseError;
use serde_json::Value;

impl TableQuery {
    /// 执行 INSERT 操作
    ///
    /// 插入数据到表中，包括以下步骤：
    /// 1. 按表定义验证所有字段值
    /// 2. 检查用户是否有字段的写入权限
    /// 3. 构建 INSERT SQL 语句
    /// 4. 执行插入操作
    /// 5. 返回影响行数
    ///
    /// # 参数
    ///
    /// - `data`：要插入的 [`crate::table::Record`]
    ///
    /// # 返回值
    ///
    /// - `Ok(u64)`：插入成功，返回影响行数（通常为 1）
    /// - `Err(BaseError)`：插入失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::FieldRequired`：必填字段缺失
    /// - `BaseError::FieldPermissionDenied`：用户无字段写入权限
    /// - `BaseError::ValidationFailed`：字段值验证失败
    /// - `BaseError::DatabaseExecuteFailed`：数据库执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::Record;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let data = Record::new()
    ///     .set("name", "张三")
    ///     .set("email", "zhangsan@example.com");
    ///
    /// // 执行插入
    /// let affected = query.insert(data).await?;
    /// println!("插入成功，影响行数: {}", affected);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn insert(self, data: crate::table::Record) -> Result<u64, BaseError> {
        // 填充默认值/时间戳并校验（顺序：写权限→填充默认值→必填/类型校验）
        let data = self.prepare_and_validate_insert(data.into_columns())?;
        self.compile_db_query()?
            .insert(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok(1)
    }

    /// 在事务中执行 INSERT 操作
    ///
    /// 与 [`TableQuery::insert`] 完全一致的写权限校验/默认值填充/时间戳/必填校验
    /// 流程，但在调用方提供的事务内执行，可与其它写操作原子提交。
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseTransactionFailed`：事务已提交/回滚
    /// - 其余同 [`TableQuery::insert`]
    pub async fn insert_in_tx(
        self,
        tx: &mut yang_db::Transaction,
        data: crate::table::Record,
    ) -> Result<u64, BaseError> {
        let data = self.prepare_and_validate_insert(data.into_columns())?;
        let query = tx.table(&self.table_config.table_ref);
        self.apply_db_plan(query)?
            .insert(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok(1)
    }

    /// 执行 INSERT 操作并返回自增主键
    ///
    /// 与 [`TableQuery::insert`] 完全一致的校验与拼 SQL 流程，但额外返回本次
    /// INSERT 产生的自增主键值（`last_insert_id`），便于调用方拿到新建记录 ID。
    ///
    /// # 参数
    ///
    /// - `data`：要插入的数据
    ///
    /// # 返回值
    ///
    /// - `Ok((affected, id))`：插入成功，返回 (影响行数, 自增主键值)。
    ///   表无自增列时 `id` 为 0。
    /// - `Err(BaseError)`：插入失败
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::FieldRequired`：必填字段缺失
    /// - `BaseError::FieldPermissionDenied`：用户无字段写入权限
    /// - `BaseError::DatabaseExecuteFailed`：数据库执行失败
    pub async fn insert_returning_id(
        self,
        data: crate::table::Record,
    ) -> Result<(u64, u64), BaseError> {
        let data = self.prepare_and_validate_insert(data.into_columns())?;
        let id = self
            .compile_db_query()?
            .insert(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok((1, id))
    }

    /// 在事务中执行 INSERT 并返回自增主键
    ///
    /// 与 [`TableQuery::insert_returning_id`] 一致，但在事务内执行。批量写入或
    /// 「插入父行→用其主键插入子行」等需要拿到新 ID 再继续的原子场景使用。
    pub async fn insert_returning_id_in_tx(
        self,
        tx: &mut yang_db::Transaction,
        data: crate::table::Record,
    ) -> Result<(u64, u64), BaseError> {
        let data = self.prepare_and_validate_insert(data.into_columns())?;
        let query = tx.table(&self.table_config.table_ref);
        let id = self
            .apply_db_plan(query)?
            .insert(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)?;
        Ok((1, id))
    }

    /// 填充默认值/时间戳并验证插入数据
    ///
    /// 处理顺序（修复 required+default 字段被误报 FieldRequired 的问题）：
    /// 1. 字段与写权限校验：显式提交只读字段时拒绝；null 自增主键视为未提供
    /// 2. 规范化数据库生成字段：未提供或为 null 的自增字段交给数据库生成
    /// 3. 填充默认值：data 中缺失且配置了 `default_value` 的字段补默认值
    /// 4. 填充时间戳：`timestamp_fields` 配置且列存在、调用方未提供时，写入当前时间
    /// 5. 必填/类型/验证器校验：在补齐后的数据上执行
    ///
    /// # 参数
    ///
    /// - `data`：调用方提供的原始插入数据
    ///
    /// # 返回值
    ///
    /// - `Ok(HashMap)`：补齐默认值与时间戳后的最终插入数据
    /// - `Err(BaseError)`：权限或校验失败
    pub(crate) fn prepare_and_validate_insert(
        &self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        let mut prepared = data;

        // tenant key 只能由请求上下文注入，业务输入不得覆盖。
        if let Some((field, value)) = &self.tenant_scope {
            if prepared.contains_key(field) {
                return Err(BaseError::PermissionDenied(format!(
                    "禁止显式写入租户字段: {field}"
                )));
            }
            prepared.insert(field.clone(), value.clone());
        }

        // 1. 校验调用方显式提交的字段和写权限。只有 null 自增主键可视为“未提供”，
        // 其余只读字段即使提交 null 也必须拒绝，避免绕过字段边界并覆盖数据库默认值。
        for (field_name, value) in &prepared {
            let field_config = self.table_config.get_field(field_name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), field_name.clone())
            })?;
            let omitted_auto_increment = field_config.auto_increment && value.is_null();
            let injected_tenant = self
                .tenant_scope
                .as_ref()
                .is_some_and(|(tenant_field, _)| tenant_field == field_name);
            if !injected_tenant
                && !omitted_auto_increment
                && !field_config.permissions.can_write(&self.user_roles_set)
            {
                return Err(BaseError::FieldPermissionDenied(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                    "用户无写入权限".to_string(),
                ));
            }
        }

        // 2. 数据库生成的自增字段未提供或为 null 时，不进入 INSERT 字段列表。
        for (field_name, field_config) in &self.table_config.fields {
            if field_config.auto_increment
                && prepared
                    .get(field_name)
                    .map(serde_json::Value::is_null)
                    .unwrap_or(true)
            {
                prepared.remove(field_name);
            }
        }

        // 3. 填充默认值（仅缺失时；显式 null 仍受 nullable/required 约束）
        for (field_name, field_config) in &self.table_config.fields {
            if field_config.auto_increment {
                continue;
            }
            if let Some(default) = &field_config.default_value {
                if !prepared.contains_key(field_name) {
                    prepared.insert(field_name.clone(), default.clone());
                }
            }
        }

        // 4. 填充创建/更新时间戳（列存在且调用方未提供时）
        if let Some(ts) = &self.table_config.timestamp_fields {
            let now = chrono::Utc::now().timestamp();
            for ts_field in [&ts.created_at, &ts.updated_at].into_iter().flatten() {
                if self.table_config.fields.contains_key(ts_field) {
                    let missing = prepared.get(ts_field).map(|v| v.is_null()).unwrap_or(true);
                    if missing {
                        prepared.insert(ts_field.clone(), Value::Number(now.into()));
                    }
                }
            }
        }

        // 5. 在补齐后的数据上执行必填/类型/验证器校验
        for (field_name, field_config) in &self.table_config.fields {
            if !field_config.permissions.can_write(&self.user_roles_set) {
                continue;
            }
            if field_config.auto_increment && !prepared.contains_key(field_name) {
                continue;
            }
            let value = prepared.get(field_name).unwrap_or(&Value::Null);
            field_config.validate(value)?;
        }

        Ok(prepared)
    }

    /// 执行 UPDATE 操作
    ///
    /// 更新表中的数据，包括以下步骤：
    /// 1. 按表定义验证所有字段值
    /// 2. 检查用户是否有字段的写入权限
    /// 3. 构建 UPDATE SQL 语句
    /// 4. 应用已配置的 WHERE 条件
    /// 5. 执行更新操作
    /// 6. 返回影响行数
    ///
    /// # 参数
    ///
    /// - `data`：要更新的 [`crate::table::Record`]
    ///
    /// # 返回值
    ///
    /// - `Ok(u64)`：更新成功，返回影响行数
    /// - `Err(BaseError)`：更新失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无字段写入权限
    /// - `BaseError::ValidationFailed`：字段值验证失败
    /// - `BaseError::DatabaseExecuteFailed`：数据库执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use yang_base::table::Record;
    /// use serde_json::json;
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let data = Record::new()
    ///     .set("name", "李四")
    ///     .set("email", "lisi@example.com");
    ///
    /// // 执行更新（需要先设置 WHERE 条件）
    /// let affected = query
    ///     .where_eq("id", json!(1))?
    ///     .update(data)
    ///     .await?;
    /// println!("更新成功，影响行数: {}", affected);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update(self, data: crate::table::Record) -> Result<u64, BaseError> {
        let data = self.prepare_update_data(data.into_columns())?;
        self.compile_db_query()?
            .update(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 在事务中执行 UPDATE 操作
    ///
    /// 与 [`TableQuery::update`] 一致的字段校验/权限/WHERE 守卫/自动 `updated_at`
    /// 逻辑，但在事务内执行。
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseTransactionFailed`：事务已提交/回滚
    /// - 其余同 [`TableQuery::update`]
    pub async fn update_in_tx(
        self,
        tx: &mut yang_db::Transaction,
        data: crate::table::Record,
    ) -> Result<u64, BaseError> {
        let data = self.prepare_update_data(data.into_columns())?;
        let query = tx.table(&self.table_config.table_ref);
        self.apply_db_plan(query)?
            .update(&data)
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    fn prepare_update_data(
        &self,
        data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        self.validate_update_data(&data)?;
        self.with_updated_timestamp(data)
    }

    fn with_updated_timestamp(
        &self,
        mut data: std::collections::HashMap<String, Value>,
    ) -> Result<std::collections::HashMap<String, Value>, BaseError> {
        if data.is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".to_string(),
                "无可更新字段".to_string(),
            ));
        }
        if let Some(updated_at) = self
            .table_config
            .timestamp_fields
            .as_ref()
            .and_then(|fields| fields.updated_at.as_ref())
            .filter(|name| self.table_config.fields.contains_key(*name))
        {
            data.insert(
                updated_at.clone(),
                Value::Number(chrono::Utc::now().timestamp().into()),
            );
        }
        Ok(data)
    }

    /// 验证更新数据
    ///
    /// 验证所有要更新的字段值的合法性和用户权限
    ///
    /// # 参数
    ///
    /// - `data`：要更新的数据
    ///
    /// # 返回值
    ///
    /// - `Ok(())`：验证通过
    /// - `Err(BaseError)`：验证失败
    ///
    /// # 错误
    ///
    /// - `BaseError::FieldNotFound`：字段不存在
    /// - `BaseError::FieldPermissionDenied`：用户无字段写入权限
    /// - `BaseError::ValidationFailed`：字段值验证失败
    #[cfg(test)]
    pub fn validate_update_data(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        self.validate_update_data_impl(data)
    }

    /// 验证更新数据（内部实现）
    #[cfg(not(test))]
    fn validate_update_data(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        self.validate_update_data_impl(data)
    }

    /// 验证更新数据的实际实现
    fn validate_update_data_impl(
        &self,
        data: &std::collections::HashMap<String, Value>,
    ) -> Result<(), BaseError> {
        if data.is_empty() {
            return Err(BaseError::ParamInvalid(
                "data".to_string(),
                "至少需要一个更新字段".to_string(),
            ));
        }
        // 只验证提供的字段（与 INSERT 不同，UPDATE 不需要验证所有字段）
        for (field_name, value) in data {
            if self
                .tenant_scope
                .as_ref()
                .is_some_and(|(tenant_field, _)| tenant_field == field_name)
            {
                return Err(BaseError::PermissionDenied(format!(
                    "禁止修改租户字段: {field_name}"
                )));
            }
            // 1. 检查字段是否存在于表配置中
            let field_config = self.table_config.get_field(field_name).ok_or_else(|| {
                BaseError::FieldNotFound(self.table_config.table_name.clone(), field_name.clone())
            })?;

            // 2. 检查用户是否有写入权限
            if !field_config.permissions.can_write(&self.user_roles_set) {
                return Err(BaseError::FieldPermissionDenied(
                    self.table_config.table_name.clone(),
                    field_name.clone(),
                    "用户无写入权限".to_string(),
                ));
            }

            // 3. 验证显式提供的字段值。部分更新不要求提交其它必填字段，但若本字段
            // 被显式设为 null，仍执行 required 约束。
            field_config.validate(value)?;
        }

        Ok(())
    }

    /// 执行 DELETE 操作
    ///
    /// 删除表中的数据，支持软删除和物理删除两种模式：
    /// 1. 如果配置了软删除字段（soft_delete_field），执行 UPDATE 设置删除标记
    /// 2. 如果未配置软删除字段，执行物理删除
    /// 3. 应用已配置的 WHERE 条件
    /// 4. 返回影响行数
    ///
    /// # 返回值
    ///
    /// - `Ok(u64)`：删除成功，返回影响行数
    /// - `Err(BaseError)`：删除失败，返回错误
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseNotInitialized`：数据库未初始化
    /// - `BaseError::DatabaseExecuteFailed`：数据库执行失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use serde_json::json;
    /// use yang_base::table::{Field, Table};
    ///
    /// # async fn example() -> Result<(), yang_base::error::BaseError> {
    /// let users = Table::new("users")
    ///     .fields(vec![
    ///         Field::id("id"),
    ///         Field::string("name", 50).required(),
    ///         Field::soft_delete("deleted_at"),
    ///     ])
    ///     .build()?;
    ///
    /// // 执行软删除（实际上是 UPDATE deleted_at = <timestamp>）
    /// let affected = users
    ///     .bind(pool)
    ///     .query(["admin"])
    ///     .where_eq("id", json!(1))?
    ///     .delete()
    ///     .await?;
    /// println!("删除成功，影响行数: {}", affected);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(self) -> Result<u64, BaseError> {
        if let Some(soft_delete_field) = &self.table_config.soft_delete_field {
            let data = self.with_updated_timestamp(std::collections::HashMap::from([(
                soft_delete_field.clone(),
                Value::Number(chrono::Utc::now().timestamp().into()),
            )]))?;
            return self
                .compile_db_query()?
                .update(&data)
                .await
                .map_err(BaseError::DatabaseExecuteFailed);
        }
        self.compile_db_query()?
            .delete()
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }

    /// 在事务中执行 DELETE 操作
    ///
    /// 与 [`TableQuery::delete`] 一致：配置了软删除字段时走 UPDATE 标记（同样在
    /// 事务内），否则物理删除；WHERE 守卫与软删语义完全复用。
    ///
    /// # 错误
    ///
    /// - `BaseError::DatabaseTransactionFailed`：事务已提交/回滚
    /// - 其余同 [`TableQuery::delete`]
    pub async fn delete_in_tx(self, tx: &mut yang_db::Transaction) -> Result<u64, BaseError> {
        let query = tx.table(&self.table_config.table_ref);
        if let Some(soft_delete_field) = &self.table_config.soft_delete_field {
            let data = self.with_updated_timestamp(std::collections::HashMap::from([(
                soft_delete_field.clone(),
                Value::Number(chrono::Utc::now().timestamp().into()),
            )]))?;
            return self
                .apply_db_plan(query)?
                .update(&data)
                .await
                .map_err(BaseError::DatabaseExecuteFailed);
        }
        self.apply_db_plan(query)?
            .delete()
            .await
            .map_err(BaseError::DatabaseExecuteFailed)
    }
}
