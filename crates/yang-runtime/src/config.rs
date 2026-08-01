//! 强类型配置的启动期来源合成。
//!
//! 文件、环境变量与目录型 secret provider 按固定优先级合成；调用方负责声明
//! 允许覆盖的字段，因此运行时不会认识任何具体应用的配置名称。

use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml::map::Map;
use toml::Value;

const DEFAULT_MAX_SECRET_BYTES: u64 = 64 * 1024;

pub type Result<T> = std::result::Result<T, ConfigSourceError>;

#[derive(Debug, Error)]
pub enum ConfigSourceError {
    #[error("{0}")]
    Invalid(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{context}: {source}")]
    Toml {
        context: &'static str,
        #[source]
        source: toml::de::Error,
    },
    #[error("{context}: {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentValueKind {
    Text,
    Integer,
    Float,
    OptionalInteger,
    Boolean,
    StringList,
    Json,
}

#[derive(Debug, Clone, Copy)]
pub struct EnvironmentBinding {
    variable: &'static str,
    section: &'static str,
    field: &'static str,
    kind: EnvironmentValueKind,
}

impl EnvironmentBinding {
    pub const fn new(
        variable: &'static str,
        section: &'static str,
        field: &'static str,
        kind: EnvironmentValueKind,
    ) -> Self {
        Self {
            variable,
            section,
            field,
            kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretValueKind {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy)]
pub struct SecretBinding {
    file_name: &'static str,
    section: &'static str,
    field: &'static str,
    kind: SecretValueKind,
}

impl SecretBinding {
    pub const fn text(file_name: &'static str, section: &'static str, field: &'static str) -> Self {
        Self {
            file_name,
            section,
            field,
            kind: SecretValueKind::Text,
        }
    }

    pub const fn json(file_name: &'static str, section: &'static str, field: &'static str) -> Self {
        Self {
            file_name,
            section,
            field,
            kind: SecretValueKind::Json,
        }
    }
}

pub trait SecretProvider {
    fn read(&self, file_name: &str) -> Result<Option<String>>;
}

#[derive(Debug, Clone, Copy)]
pub struct ConfigSources {
    environment_prefix: &'static str,
    secret_directory_variable: &'static str,
    environment_bindings: &'static [EnvironmentBinding],
    secret_bindings: &'static [SecretBinding],
    ignored_environment_prefixes: &'static [&'static str],
    max_secret_bytes: u64,
}

impl ConfigSources {
    pub const fn new(
        environment_prefix: &'static str,
        secret_directory_variable: &'static str,
        environment_bindings: &'static [EnvironmentBinding],
        secret_bindings: &'static [SecretBinding],
    ) -> Self {
        Self {
            environment_prefix,
            secret_directory_variable,
            environment_bindings,
            secret_bindings,
            ignored_environment_prefixes: &[],
            max_secret_bytes: DEFAULT_MAX_SECRET_BYTES,
        }
    }

    #[must_use]
    pub const fn with_ignored_environment_prefixes(
        mut self,
        prefixes: &'static [&'static str],
    ) -> Self {
        self.ignored_environment_prefixes = prefixes;
        self
    }

    #[must_use]
    pub const fn with_max_secret_bytes(mut self, max_secret_bytes: u64) -> Self {
        self.max_secret_bytes = max_secret_bytes;
        self
    }

    pub fn load<T>(&self, path: &Path, read_context: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let raw = std::fs::read_to_string(path).map_err(|source| ConfigSourceError::Io {
            context: format!("{read_context}: {}", path.display()),
            source,
        })?;
        let environment = self.collect_environment_variables(std::env::vars_os())?;
        let provider = self.process_secret_provider()?;
        self.parse_with_sources(
            &raw,
            &environment,
            provider.as_ref().map(|value| value as _),
        )
    }

    pub fn parse_file_only<T>(&self, raw: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.parse_with_sources(raw, &BTreeMap::new(), None)
    }

    pub fn parse_with_sources<T>(
        &self,
        raw: &str,
        environment: &BTreeMap<String, String>,
        provider: Option<&dyn SecretProvider>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut document: Value =
            toml::from_str(raw).map_err(|source| ConfigSourceError::Toml {
                context: "解析配置文件失败",
                source,
            })?;
        if !document.is_table() {
            return Err(invalid("配置文件顶层必须是 TOML table"));
        }
        self.apply_environment(&mut document, environment)?;
        if let Some(provider) = provider {
            self.apply_secrets(&mut document, provider)?;
        }
        document
            .try_into()
            .map_err(|source| ConfigSourceError::Toml {
                context: "解析配置文件失败",
                source,
            })
    }

    pub fn collect_environment_variables<I>(&self, variables: I) -> Result<BTreeMap<String, String>>
    where
        I: IntoIterator<Item = (OsString, OsString)>,
    {
        let mut environment = BTreeMap::new();
        for (name, value) in variables {
            let Ok(name) = name.into_string() else {
                continue;
            };
            if !name.starts_with(self.environment_prefix)
                || name == self.secret_directory_variable
                || self
                    .ignored_environment_prefixes
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
            {
                continue;
            }
            let Some(binding) = self
                .environment_bindings
                .iter()
                .find(|binding| binding.variable == name)
            else {
                return Err(invalid(format!("不支持的环境变量: {name}")));
            };
            environment.insert(
                binding.variable.to_owned(),
                unicode_environment(&name, value)?,
            );
        }
        Ok(environment)
    }

    fn process_secret_provider(&self) -> Result<Option<DirectorySecretProvider>> {
        let Some(value) = std::env::var_os(self.secret_directory_variable) else {
            return Ok(None);
        };
        let directory = unicode_environment(self.secret_directory_variable, value)?;
        if directory.trim().is_empty() {
            return Err(invalid(format!(
                "{} 不能为空",
                self.secret_directory_variable
            )));
        }
        DirectorySecretProvider::new(PathBuf::from(directory), self.max_secret_bytes).map(Some)
    }

    fn apply_environment(
        &self,
        document: &mut Value,
        environment: &BTreeMap<String, String>,
    ) -> Result<()> {
        for binding in self.environment_bindings {
            let Some(raw) = environment.get(binding.variable) else {
                continue;
            };
            match parse_environment_value(binding, raw)? {
                ResolvedOverride::Set(value) => {
                    set_value(document, binding.section, binding.field, value)?;
                }
                ResolvedOverride::Remove => {
                    remove_value(document, binding.section, binding.field)?;
                }
            }
        }
        Ok(())
    }

    fn apply_secrets(&self, document: &mut Value, provider: &dyn SecretProvider) -> Result<()> {
        for binding in self.secret_bindings {
            if let Some(value) = provider.read(binding.file_name)? {
                let value = match binding.kind {
                    SecretValueKind::Text => Value::String(value),
                    SecretValueKind::Json => parse_json_override(binding.file_name, &value)?,
                };
                set_value(document, binding.section, binding.field, value)?;
            }
        }
        Ok(())
    }
}

struct DirectorySecretProvider {
    directory: PathBuf,
    max_secret_bytes: u64,
}

impl DirectorySecretProvider {
    fn new(directory: PathBuf, max_secret_bytes: u64) -> Result<Self> {
        let metadata = std::fs::metadata(&directory).map_err(|source| ConfigSourceError::Io {
            context: format!("secret 目录不可访问: {}", directory.display()),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(invalid(format!(
                "secret provider 必须指向目录: {}",
                directory.display()
            )));
        }
        Ok(Self {
            directory,
            max_secret_bytes,
        })
    }
}

impl SecretProvider for DirectorySecretProvider {
    fn read(&self, file_name: &str) -> Result<Option<String>> {
        let path = self.directory.join(file_name);
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(ConfigSourceError::Io {
                    context: format!("读取 secret 文件失败: {}", path.display()),
                    source,
                });
            }
        };
        read_secret(file, &path, self.max_secret_bytes).map(Some)
    }
}

fn read_secret(file: File, path: &Path, max_secret_bytes: u64) -> Result<String> {
    let mut bytes = Vec::new();
    file.take(max_secret_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| ConfigSourceError::Io {
            context: format!("读取 secret 文件失败: {}", path.display()),
            source,
        })?;
    if bytes.len() as u64 > max_secret_bytes {
        return Err(invalid(format!(
            "secret 文件超过 {max_secret_bytes} 字节上限: {}",
            path.display()
        )));
    }
    let value = String::from_utf8(bytes)
        .map_err(|_| invalid(format!("secret 文件必须是 UTF-8 文本: {}", path.display())))?;
    let value = value
        .strip_suffix("\r\n")
        .or_else(|| value.strip_suffix('\n'))
        .or_else(|| value.strip_suffix('\r'))
        .unwrap_or(&value);
    if value.is_empty() {
        return Err(invalid(format!("secret 文件不能为空: {}", path.display())));
    }
    if value
        .chars()
        .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        return Err(invalid(format!(
            "secret 文件只能包含单行文本: {}",
            path.display()
        )));
    }
    Ok(value.to_owned())
}

enum ResolvedOverride {
    Set(Value),
    Remove,
}

fn parse_environment_value(binding: &EnvironmentBinding, raw: &str) -> Result<ResolvedOverride> {
    let value = match binding.kind {
        EnvironmentValueKind::Text => Value::String(raw.to_owned()),
        EnvironmentValueKind::Integer => {
            Value::Integer(parse_non_negative_integer(binding.variable, raw)?)
        }
        EnvironmentValueKind::Float => Value::Float(
            raw.trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    invalid(format!("环境变量 {} 必须是有限浮点数", binding.variable))
                })?,
        ),
        EnvironmentValueKind::OptionalInteger
            if raw.trim().is_empty() || raw.trim().eq_ignore_ascii_case("none") =>
        {
            return Ok(ResolvedOverride::Remove);
        }
        EnvironmentValueKind::OptionalInteger => {
            Value::Integer(parse_non_negative_integer(binding.variable, raw)?)
        }
        EnvironmentValueKind::Boolean => Value::Boolean(match raw.trim() {
            "true" => true,
            "false" => false,
            _ => {
                return Err(invalid(format!(
                    "环境变量 {} 必须是小写 true 或 false",
                    binding.variable
                )));
            }
        }),
        EnvironmentValueKind::StringList => Value::Array(if raw.trim().is_empty() {
            Vec::new()
        } else {
            raw.split(',')
                .map(str::trim)
                .map(|item| {
                    if item.is_empty() {
                        Err(invalid(format!(
                            "环境变量 {} 包含空列表项",
                            binding.variable
                        )))
                    } else {
                        Ok(Value::String(item.to_owned()))
                    }
                })
                .collect::<Result<Vec<_>>>()?
        }),
        EnvironmentValueKind::Json => parse_json_override(binding.variable, raw)?,
    };
    Ok(ResolvedOverride::Set(value))
}

fn parse_non_negative_integer(variable: &str, raw: &str) -> Result<i64> {
    raw.trim()
        .parse::<i64>()
        .ok()
        .filter(|value| *value >= 0)
        .ok_or_else(|| invalid(format!("环境变量 {variable} 必须是非负十进制整数")))
}

fn parse_json_override(source_name: &str, raw: &str) -> Result<Value> {
    let json: serde_json::Value =
        serde_json::from_str(raw).map_err(|source| ConfigSourceError::Json {
            context: format!("{source_name} 必须是合法 JSON"),
            source,
        })?;
    Value::try_from(json).map_err(|error| {
        invalid(format!(
            "{source_name} JSON 不能转换为 TOML 配置值: {error}"
        ))
    })
}

fn set_value(document: &mut Value, section: &str, field: &str, value: Value) -> Result<()> {
    let table = section_table_mut(document, section, true)?
        .ok_or_else(|| invalid(format!("配置项 {section} 必须是 TOML table")))?;
    table.insert(field.to_owned(), value);
    Ok(())
}

fn remove_value(document: &mut Value, section: &str, field: &str) -> Result<()> {
    if let Some(table) = section_table_mut(document, section, false)? {
        table.remove(field);
    }
    Ok(())
}

fn section_table_mut<'a>(
    document: &'a mut Value,
    section: &str,
    create: bool,
) -> Result<Option<&'a mut Map<String, Value>>> {
    let mut current = document;
    for segment in section.split('.') {
        let table = current
            .as_table_mut()
            .ok_or_else(|| invalid(format!("配置项 {section} 必须是 TOML table")))?;
        if !table.contains_key(segment) && !create {
            return Ok(None);
        }
        current = table
            .entry(segment.to_owned())
            .or_insert_with(|| Value::Table(Map::new()));
    }
    current
        .as_table_mut()
        .map(Some)
        .ok_or_else(|| invalid(format!("配置项 {section} 必须是 TOML table")))
}

fn unicode_environment(variable: &str, value: OsString) -> Result<String> {
    value
        .into_string()
        .map_err(|_| invalid(format!("环境变量 {variable} 必须是 Unicode 文本")))
}

fn invalid(message: impl Into<String>) -> ConfigSourceError {
    ConfigSourceError::Invalid(message.into())
}
