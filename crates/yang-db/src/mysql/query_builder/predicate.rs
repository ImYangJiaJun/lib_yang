//! 服务端谓词（[`crate::Predicate`]）到条件树的受控转换。

use crate::mysql::condition::{Condition, SqlValue};

pub(crate) fn predicate_condition(
    predicate: &crate::Predicate,
) -> Result<Condition, crate::error::DbError> {
    let condition = match predicate {
        crate::Predicate::Compare(field, operator, value) => {
            let name = field.as_str().to_string();
            if matches!(operator, crate::CompareOp::Like) {
                return value
                    .as_str()
                    .map(|pattern| Condition::Like(name, pattern.to_string()))
                    .ok_or_else(|| {
                        crate::error::DbError::InvalidArgument(
                            "LIKE predicate requires string value".to_string(),
                        )
                    });
            }
            let value = predicate_value(value);
            match operator {
                crate::CompareOp::Eq => Condition::Eq(name, value),
                crate::CompareOp::Ne => Condition::Ne(name, value),
                crate::CompareOp::Gt => Condition::Gt(name, value),
                crate::CompareOp::Lt => Condition::Lt(name, value),
                crate::CompareOp::Gte => Condition::Gte(name, value),
                crate::CompareOp::Lte => Condition::Lte(name, value),
                crate::CompareOp::Like => {
                    return Err(crate::error::DbError::InvalidArgument(
                        "LIKE predicate normalization failed".to_string(),
                    ));
                }
            }
        }
        crate::Predicate::In(field, values) => Condition::In(
            field.as_str().to_string(),
            values.iter().map(predicate_value).collect(),
        ),
        crate::Predicate::NotIn(field, values) => Condition::NotIn(
            field.as_str().to_string(),
            values.iter().map(predicate_value).collect(),
        ),
        crate::Predicate::Between(field, start, end) => Condition::Between(
            field.as_str().to_string(),
            predicate_value(start),
            predicate_value(end),
        ),
        crate::Predicate::IsNull(field) => Condition::IsNull(field.as_str().to_string()),
        crate::Predicate::IsNotNull(field) => Condition::IsNotNull(field.as_str().to_string()),
        crate::Predicate::And(values) => Condition::And(
            values
                .iter()
                .map(predicate_condition)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        crate::Predicate::Or(values) => Condition::Or(
            values
                .iter()
                .map(predicate_condition)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    };
    Ok(condition)
}

pub(crate) fn predicate_value(value: &serde_json::Value) -> SqlValue {
    match value {
        serde_json::Value::Null => SqlValue::Null,
        serde_json::Value::Bool(value) => SqlValue::Bool(*value),
        serde_json::Value::Number(value) => {
            value.as_i64().map(SqlValue::Int).unwrap_or_else(|| {
                value
                    .as_u64()
                    .map(SqlValue::from)
                    .or_else(|| value.as_f64().map(SqlValue::Float))
                    .unwrap_or_else(|| SqlValue::String(value.to_string()))
            })
        }
        serde_json::Value::String(value) => SqlValue::String(value.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => SqlValue::Json(value.clone()),
    }
}
