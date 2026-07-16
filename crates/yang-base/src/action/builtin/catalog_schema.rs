//! 内置 CRUD Action 的表绑定描述契约。

use super::add::{AffectedResult, InsertResult};
use super::get::GetByPk;
use super::put::PutInput;
use super::select::{SelectQuery, SelectResult};
use super::table::{EmptyInput, TableSchemaResponse};
use crate::action::PermissionMode;
use crate::error::BaseError;
use crate::table::TableDefinition;
use schemars::schema::RootSchema;
use serde_json::{json, Value};

/// `.crud()` 为一个内置 Action 绑定的运行时描述与授权契约。
#[derive(Debug, Clone)]
pub(crate) struct BuiltinActionContract {
    pub(crate) input_schema: RootSchema,
    pub(crate) output_schema: RootSchema,
    pub(crate) permissions: Vec<String>,
    pub(crate) permission_mode: PermissionMode,
}

/// 为六个内置 CRUD Action 生成与具体表定义绑定的契约。
pub(crate) fn crud_contracts(
    definition: &TableDefinition,
    module_name: &str,
) -> Result<Vec<(&'static str, BuiltinActionContract)>, BaseError> {
    let create_schema = definition.input_schema();
    let record_schema = definition.output_schema();
    let update_schema = update_schema(&create_schema, definition.label())?;
    let primary_key_schema = definition
        .field_schema(definition.primary_key())
        .ok_or_else(|| {
            BaseError::ConfigError(format!(
                "表 {} 缺少主键字段 schema: {}",
                definition.name(),
                definition.primary_key()
            ))
        })?;

    let write_permission = format!("{module_name}:write");
    let read_permission = format!("{module_name}:read");

    Ok(vec![
        (
            "add",
            contract(
                root_schema(create_schema.clone(), "add input")?,
                schemars::schema_for!(InsertResult),
                write_permission.clone(),
            ),
        ),
        (
            "put",
            contract(
                put_input_schema(primary_key_schema.clone(), update_schema)?,
                schemars::schema_for!(AffectedResult),
                write_permission.clone(),
            ),
        ),
        (
            "del",
            contract(
                primary_key_input_schema::<GetByPk>(primary_key_schema.clone(), "del")?,
                schemars::schema_for!(AffectedResult),
                write_permission,
            ),
        ),
        (
            "get",
            contract(
                primary_key_input_schema::<GetByPk>(primary_key_schema, "get")?,
                root_schema(record_schema.clone(), "get output")?,
                read_permission.clone(),
            ),
        ),
        (
            "select",
            contract(
                select_input_schema(definition)?,
                select_output_schema(record_schema.clone())?,
                read_permission.clone(),
            ),
        ),
        (
            "table",
            contract(
                schemars::schema_for!(EmptyInput),
                table_output_schema(definition, create_schema, record_schema)?,
                read_permission,
            ),
        ),
    ])
}

fn contract(
    input_schema: RootSchema,
    output_schema: RootSchema,
    permission: String,
) -> BuiltinActionContract {
    BuiltinActionContract {
        input_schema,
        output_schema,
        permissions: vec![permission],
        permission_mode: PermissionMode::All,
    }
}

fn update_schema(create_schema: &Value, table_label: &str) -> Result<Value, BaseError> {
    let mut schema = create_schema.clone();
    let object = schema
        .as_object_mut()
        .ok_or_else(|| BaseError::ConfigError("表写入 schema 必须是 object".to_string()))?;
    object.insert("title".to_string(), json!(format!("{table_label}更新字段")));
    object.insert("required".to_string(), json!([]));
    object.insert("minProperties".to_string(), json!(1));
    Ok(schema)
}

fn put_input_schema(
    primary_key_schema: Value,
    update_schema: Value,
) -> Result<RootSchema, BaseError> {
    let mut schema = schema_value(schemars::schema_for!(PutInput), "put input")?;
    replace_root_property(&mut schema, "id", primary_key_schema, "put input")?;
    replace_root_property(&mut schema, "data", update_schema, "put input")?;
    root_schema(schema, "put input")
}

fn primary_key_input_schema<T: schemars::JsonSchema>(
    primary_key_schema: Value,
    label: &str,
) -> Result<RootSchema, BaseError> {
    let mut schema = schema_value(schemars::schema_for!(T), label)?;
    replace_root_property(&mut schema, "id", primary_key_schema, label)?;
    root_schema(schema, label)
}

fn select_input_schema(definition: &TableDefinition) -> Result<RootSchema, BaseError> {
    let mut schema = schema_value(schemars::schema_for!(SelectQuery), "select input")?;
    let filterable = definition
        .fields()
        .into_iter()
        .filter(|field| field.is_filterable() && !field.is_secret())
        .map(|field| field.name().to_string())
        .collect::<Vec<_>>();
    let sortable = definition
        .fields()
        .into_iter()
        .filter(|field| field.is_sortable() && !field.is_secret())
        .map(|field| field.name().to_string())
        .collect::<Vec<_>>();

    replace_definition_fields(&mut schema, "WhereCondition", &filterable)?;
    replace_definition_fields(&mut schema, "OrderByItem", &sortable)?;
    root_schema(schema, "select input")
}

fn select_output_schema(record_schema: Value) -> Result<RootSchema, BaseError> {
    let mut schema = schema_value(schemars::schema_for!(SelectResult), "select output")?;
    let items = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut("items"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            BaseError::ConfigError("select output 缺少 items 数组 schema".to_string())
        })?;
    items.insert("items".to_string(), record_schema);
    root_schema(schema, "select output")
}

fn table_output_schema(
    definition: &TableDefinition,
    input_schema: Value,
    output_schema: Value,
) -> Result<RootSchema, BaseError> {
    // 先生成静态类型 schema，确保 TableSchemaResponse 的字段契约变化会被编译器覆盖；
    // 随后将四个运行时值收紧为当前表的常量。
    let mut schema = schema_value(schemars::schema_for!(TableSchemaResponse), "table output")?;
    replace_root_property(
        &mut schema,
        "table_name",
        json!({ "type": "string", "const": definition.name() }),
        "table output",
    )?;
    replace_root_property(
        &mut schema,
        "primary_key",
        json!({ "type": "string", "const": definition.primary_key() }),
        "table output",
    )?;
    replace_root_property(
        &mut schema,
        "input_schema",
        json!({ "const": input_schema }),
        "table output",
    )?;
    replace_root_property(
        &mut schema,
        "output_schema",
        json!({ "const": output_schema }),
        "table output",
    )?;
    root_schema(schema, "table output")
}

fn replace_root_property(
    schema: &mut Value,
    property: &str,
    replacement: Value,
    label: &str,
) -> Result<(), BaseError> {
    let properties = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| BaseError::ConfigError(format!("{label} 缺少 properties schema")))?;
    if !properties.contains_key(property) {
        return Err(BaseError::ConfigError(format!(
            "{label} 缺少 {property} schema"
        )));
    }
    properties.insert(property.to_string(), replacement);
    Ok(())
}

fn replace_definition_fields(
    schema: &mut Value,
    definition: &str,
    fields: &[String],
) -> Result<(), BaseError> {
    let target = schema
        .get_mut("definitions")
        .and_then(Value::as_object_mut)
        .and_then(|definitions| definitions.get_mut(definition))
        .ok_or_else(|| {
            BaseError::ConfigError(format!("select input 缺少 {definition} definition"))
        })?;
    let replacement = selector_schema(fields);
    let replaced = replace_field_properties(target, &replacement);
    if replaced == 0 {
        return Err(BaseError::ConfigError(format!(
            "select input 的 {definition} 未包含 field schema"
        )));
    }
    Ok(())
}

fn selector_schema(fields: &[String]) -> Value {
    if fields.is_empty() {
        Value::Bool(false)
    } else {
        json!({ "type": "string", "enum": fields })
    }
}

fn replace_field_properties(value: &mut Value, replacement: &Value) -> usize {
    match value {
        Value::Object(object) => {
            let mut replaced = 0;
            if let Some(properties) = object.get_mut("properties").and_then(Value::as_object_mut) {
                if properties.contains_key("field") {
                    properties.insert("field".to_string(), replacement.clone());
                    replaced += 1;
                }
            }
            replaced
                + object
                    .values_mut()
                    .map(|child| replace_field_properties(child, replacement))
                    .sum::<usize>()
        }
        Value::Array(values) => values
            .iter_mut()
            .map(|child| replace_field_properties(child, replacement))
            .sum(),
        _ => 0,
    }
}

fn schema_value(schema: RootSchema, label: &str) -> Result<Value, BaseError> {
    serde_json::to_value(schema)
        .map_err(|error| BaseError::JsonSerializeFailed(format!("{label} schema: {error}")))
}

fn root_schema(schema: Value, label: &str) -> Result<RootSchema, BaseError> {
    serde_json::from_value(schema)
        .map_err(|error| BaseError::JsonDeserializeFailed(format!("{label} schema: {error}")))
}
