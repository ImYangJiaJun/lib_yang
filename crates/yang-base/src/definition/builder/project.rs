//! 请求级 UI 投影辅助：列读写可见性、表单字段、树与查询 schema。

use crate::action::ActionContext;

use super::registry::{RuntimeTableColumn, RuntimeTableView};

pub(super) fn project_tree(
    view: &RuntimeTableView,
    context: &ActionContext,
) -> Option<crate::definition::TreeViewSchema> {
    let tree = view.tree.as_ref()?;
    tree.fields
        .iter()
        .all(|column| column_readable(column, context))
        .then(|| tree.schema.clone())
}

pub(super) fn project_table_query(
    view: &RuntimeTableView,
    context: &ActionContext,
) -> crate::definition::TableQuerySchema {
    let readable_columns = view
        .columns
        .iter()
        .filter(|column| column_readable(column, context));
    let search_fields = readable_columns
        .clone()
        .filter(|column| column.schema.searchable)
        .map(|column| column.schema.field.clone())
        .collect();
    let filter_fields = readable_columns
        .filter(|column| column.schema.filterable)
        .map(|column| column.schema.field.clone())
        .collect();
    let default_sort = view
        .default_sort
        .iter()
        .filter(|sort| column_readable(&sort.column, context))
        .map(|sort| sort.schema.clone())
        .collect();
    crate::definition::TableQuerySchema {
        search_fields,
        filter_fields,
        default_sort,
        default_page_size: crate::table::DEFAULT_QUERY_PAGE_SIZE,
        max_page_size: crate::table::MAX_TABLE_QUERY_PAGE_SIZE,
    }
}

pub(super) fn table_column_schema(
    column: &RuntimeTableColumn,
    context: &ActionContext,
) -> crate::definition::TableColumnSchema {
    let mut schema = column.schema.clone();
    schema.relation = column
        .relation
        .as_ref()
        .filter(|relation| relation.policy.allows(context))
        .map(|relation| relation.schema.clone());
    schema
}

pub(super) fn column_readable(column: &RuntimeTableColumn, context: &ActionContext) -> bool {
    !column.secret && access_rule_allows(&column.readable, context)
}

pub(super) fn form_field(
    column: &RuntimeTableColumn,
    context: &ActionContext,
) -> Option<crate::definition::FormFieldSchema> {
    let readable = column_readable(column, context);
    let writable = !column.server_managed && access_rule_allows(&column.writable, context);
    if !readable && !writable {
        return None;
    }
    Some(crate::definition::FormFieldSchema {
        field: column.schema.field.clone(),
        title: column.schema.title.clone(),
        description: column.schema.description.clone(),
        widget: column.schema.widget,
        required: column.schema.required && writable,
        read_only: !writable,
        write_only: column.secret || !readable,
        relation: column
            .relation
            .as_ref()
            .filter(|relation| relation.policy.allows(context))
            .map(|relation| relation.schema.clone()),
        validation: column.validation.clone(),
    })
}

pub(super) fn access_rule_allows(
    rule: &crate::definition::AccessRule,
    context: &ActionContext,
) -> bool {
    match rule {
        crate::definition::AccessRule::Everyone => true,
        crate::definition::AccessRule::Nobody => false,
        crate::definition::AccessRule::Roles(roles) => context
            .user_roles_set()
            .is_some_and(|user_roles| roles.iter().any(|role| user_roles.contains(role))),
    }
}
