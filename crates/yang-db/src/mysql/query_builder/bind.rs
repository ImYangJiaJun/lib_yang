//! `SqlValue` 到 sqlx 查询的参数绑定助手。

use crate::mysql::condition::SqlValue;

/// 将 SqlValue 绑定到 sqlx 查询的内部宏
///
/// 封装 SqlValue 各变体到 `.bind()` 调用的映射逻辑，消除 4 个 bind_param
/// 函数中完全相同的 match 分支重复代码。未来新增 SqlValue 变体时，
/// 只需在此宏中添加一个分支即可完成所有函数的更新。
///
/// # 参数
/// - `$query`: sqlx 查询对象（支持 `.bind()` 方法的任意类型）
/// - `$param`: `&SqlValue` 引用
///
/// # 返回
/// 绑定参数后的查询对象（与 `$query` 类型相同）
macro_rules! bind_value_match {
    ($query:expr, $param:expr) => {
        match $param {
            // NULL 值：绑定为 Option<i32>::None
            SqlValue::Null => $query.bind(Option::<i32>::None),
            // 布尔值
            SqlValue::Bool(b) => $query.bind(*b),
            // 整数
            SqlValue::Int(i) => $query.bind(*i),
            // 浮点数
            SqlValue::Float(f) => $query.bind(*f),
            // 字符串（需要 clone 以满足 sqlx 的所有权要求）
            SqlValue::String(s) => $query.bind(s.clone()),
            // 字节数组（需要 clone）
            SqlValue::Bytes(b) => $query.bind(b.clone()),
            // JSON 值：序列化为字符串后绑定
            SqlValue::Json(j) => $query.bind(j.to_string()),
            // 日期时间
            SqlValue::DateTime(dt) => $query.bind(*dt),
            // 时间戳（整数）
            SqlValue::Timestamp(ts) => $query.bind(*ts),
        }
    };
}

/// 绑定参数到执行查询（用于 INSERT/UPDATE/DELETE）
///
/// # 参数
/// - query: sqlx 查询对象
/// - param: SQL 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
pub(super) fn bind_execute_param<'q>(
    query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    param: &SqlValue,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
    // 使用 bind_value_match! 宏统一处理 SqlValue 各变体的绑定逻辑
    bind_value_match!(query, param)
}

/// 绑定参数到查询
///
/// # 参数
/// - query: sqlx 查询对象
/// - param: SQL 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
pub(crate) fn bind_param<'q, T>(
    query: sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
    param: &SqlValue,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    // 使用 bind_value_match! 宏统一处理 SqlValue 各变体的绑定逻辑
    bind_value_match!(query, param)
}

/// 绑定参数到标量查询
///
/// # 参数
/// - query: sqlx 标量查询对象
/// - param: SQL 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
pub(super) fn bind_scalar_param<'q, T>(
    query: sqlx::query::QueryScalar<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
    param: &SqlValue,
) -> sqlx::query::QueryScalar<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
where
    T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
{
    // 使用 bind_value_match! 宏统一处理 SqlValue 各变体的绑定逻辑
    bind_value_match!(query, param)
}

/// 绑定参数到标量查询（Option 类型）
///
/// # 参数
/// - query: sqlx 标量查询对象（返回 Option<T>）
/// - param: SQL 参数值
///
/// # 返回
/// - 绑定参数后的查询对象
///
/// 注：聚合/标量方法现统一走 `fetch_scalar` + `bind_scalar_param`（后者对 `Option<T>`
/// 输出类型同样适用），本函数暂无调用方但保留作为公开内部表面，标注 allow(dead_code)。
#[allow(dead_code)]
pub(super) fn bind_scalar_param_option<'q, T>(
    query: sqlx::query::QueryScalar<'q, sqlx::MySql, Option<T>, sqlx::mysql::MySqlArguments>,
    param: &SqlValue,
) -> sqlx::query::QueryScalar<'q, sqlx::MySql, Option<T>, sqlx::mysql::MySqlArguments>
where
    T: for<'r> sqlx::Decode<'r, sqlx::MySql> + sqlx::Type<sqlx::MySql> + Send + Unpin,
{
    // 使用 bind_value_match! 宏统一处理 SqlValue 各变体的绑定逻辑
    bind_value_match!(query, param)
}
