//! 构建期名称与受控引用。

use super::BuildError;
use std::fmt;

#[derive(Clone, Copy)]
enum NameRule {
    Segment,
    Qualified,
    SqlIdentifier,
}

fn validate_name(kind: &'static str, value: &str, rule: NameRule) -> Result<(), BuildError> {
    let valid_segment = |segment: &str| {
        let mut chars = segment.chars();
        matches!(chars.next(), Some(first) if first.is_ascii_lowercase())
            && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    };

    let valid = match rule {
        NameRule::Segment | NameRule::SqlIdentifier => valid_segment(value),
        NameRule::Qualified => {
            let mut segments = value.split('.');
            let first = segments.next();
            let second = segments.next();
            first.is_some()
                && second.is_some()
                && first
                    .into_iter()
                    .chain(second)
                    .chain(segments)
                    .all(valid_segment)
        }
    };

    if valid {
        Ok(())
    } else {
        Err(BuildError::InvalidName {
            kind,
            name: value.to_string(),
        })
    }
}

macro_rules! define_name {
    ($name:ident, $kind:literal, $rule:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// 校验并创建名称。
            pub fn new(value: impl Into<String>) -> Result<Self, BuildError> {
                let value = value.into();
                validate_name($kind, &value, $rule)?;
                Ok(Self(value))
            }

            /// 返回规范名称字符串。
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

define_name!(AddonName, "Addon", NameRule::Segment, "Addon 的受控名称。");
define_name!(
    ModuleName,
    "Module",
    NameRule::Qualified,
    "包含 Addon 前缀的 Module 受控名称，例如 `org.user`。"
);
define_name!(
    ActionName,
    "Action",
    NameRule::Segment,
    "Action 的局部受控名称。"
);
define_name!(
    TableName,
    "Table",
    NameRule::SqlIdentifier,
    "数据库表的受控名称。"
);
define_name!(FieldName, "Field", NameRule::Segment, "字段的受控名称。");
define_name!(ViewName, "View", NameRule::Segment, "View 的局部受控名称。");

/// 指向已声明表字段的构建期引用。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldRef {
    table: TableName,
    field: FieldName,
}

impl FieldRef {
    /// 从受控表名和字段名创建引用。
    pub fn new(table: TableName, field: FieldName) -> Self {
        Self { table, field }
    }

    /// 返回目标表名。
    pub fn table(&self) -> &TableName {
        &self.table
    }

    /// 返回目标字段名。
    pub fn field(&self) -> &FieldName {
        &self.field
    }
}

impl fmt::Display for FieldRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.table, self.field)
    }
}

impl FieldName {
    /// 由已经通过宏内 const 校验的字段字面量创建。
    #[doc(hidden)]
    pub fn __from_validated_literal(value: &'static str) -> Self {
        Self(value.to_string())
    }
}

impl ActionName {
    /// 由已经通过派生宏校验的 Action 名字面量创建。
    #[doc(hidden)]
    pub fn __from_validated_literal(value: &'static str) -> Self {
        Self(value.to_string())
    }
}

macro_rules! validated_name_literal {
    ($name:ident) => {
        impl $name {
            #[doc(hidden)]
            pub fn __from_validated_literal(value: &'static str) -> Self {
                Self(value.to_string())
            }
        }
    };
}

validated_name_literal!(AddonName);
validated_name_literal!(ModuleName);
validated_name_literal!(TableName);
validated_name_literal!(ViewName);

/// 编译期校验单段业务名称。
#[doc(hidden)]
pub const fn __validate_segment_literal(value: &str) {
    __validate_field_literal(value);
}

/// 编译期校验点分隔的业务名称。
#[doc(hidden)]
pub const fn __validate_qualified_literal(value: &str) {
    let bytes = value.as_bytes();
    assert!(!bytes.is_empty(), "qualified name cannot be empty");
    let mut start = 0;
    let mut index = 0;
    while index <= bytes.len() {
        if index == bytes.len() || bytes[index] == b'.' {
            assert!(index > start, "qualified name segment cannot be empty");
            let mut part = start;
            while part < index {
                let byte = bytes[part];
                if part == start {
                    assert!(
                        byte.is_ascii_lowercase(),
                        "name segment must start lowercase"
                    );
                } else {
                    assert!(
                        byte == b'_' || byte.is_ascii_lowercase() || byte.is_ascii_digit(),
                        "name segment must be snake_case ASCII"
                    );
                }
                part += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
}

/// 编译期校验至少包含一个限定分隔符的引用。
#[doc(hidden)]
pub const fn __validate_ref_literal(value: &str) {
    __validate_qualified_literal(value);
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut found = false;
    while index < bytes.len() {
        if bytes[index] == b'.' {
            found = true;
        }
        index += 1;
    }
    assert!(found, "reference must contain a qualifier");
}

impl ActionRef {
    #[doc(hidden)]
    pub fn __from_validated_literal(value: &'static str) -> Self {
        match value.rsplit_once('.') {
            Some((module, action)) => Self {
                module: ModuleName(module.to_string()),
                action: ActionName(action.to_string()),
            },
            None => Self {
                module: ModuleName(String::new()),
                action: ActionName(String::new()),
            },
        }
    }
}

impl FieldRef {
    #[doc(hidden)]
    pub fn __from_validated_literal(value: &'static str) -> Self {
        match value.rsplit_once('.') {
            Some((table, field)) => Self {
                table: TableName(table.to_string()),
                field: FieldName(field.to_string()),
            },
            None => Self {
                table: TableName(String::new()),
                field: FieldName(String::new()),
            },
        }
    }
}

impl ViewRef {
    #[doc(hidden)]
    pub fn __from_validated_literal(value: &'static str) -> Self {
        match value.rsplit_once('.') {
            Some((module, view)) => Self {
                module: ModuleName(module.to_string()),
                view: ViewName(view.to_string()),
            },
            None => Self {
                module: ModuleName(String::new()),
                view: ViewName(String::new()),
            },
        }
    }
}

/// 编译期校验 fields!/params! 中的 snake_case 字段名。
#[doc(hidden)]
pub const fn __validate_field_literal(value: &str) {
    let bytes = value.as_bytes();
    assert!(!bytes.is_empty(), "field name cannot be empty");
    let first = bytes[0];
    assert!(
        first.is_ascii_lowercase(),
        "field name must start with lowercase ASCII letter"
    );
    let mut index = 1;
    while index < bytes.len() {
        let byte = bytes[index];
        assert!(
            byte == b'_' || byte.is_ascii_lowercase() || byte.is_ascii_digit(),
            "field name must be snake_case ASCII"
        );
        index += 1;
    }
}

/// 指向已声明 Action 的构建期引用。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionRef {
    module: ModuleName,
    action: ActionName,
}

impl ActionRef {
    /// 从受控 Module 与 Action 名称创建引用。
    pub fn new(module: ModuleName, action: ActionName) -> Self {
        Self { module, action }
    }

    /// 返回目标 Module 名称。
    pub fn module(&self) -> &ModuleName {
        &self.module
    }

    /// 返回目标 Action 名称。
    pub fn action(&self) -> &ActionName {
        &self.action
    }
}

impl fmt::Display for ActionRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.module, self.action)
    }
}

/// 指向已声明 View 的构建期引用。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViewRef {
    module: ModuleName,
    view: ViewName,
}

impl ViewRef {
    /// 从受控 Module 与 View 名称创建引用。
    pub fn new(module: ModuleName, view: ViewName) -> Self {
        Self { module, view }
    }

    /// 返回目标 Module 名称。
    pub fn module(&self) -> &ModuleName {
        &self.module
    }

    /// 返回目标 View 名称。
    pub fn view(&self) -> &ViewName {
        &self.view
    }
}

impl fmt::Display for ViewRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.module, self.view)
    }
}
