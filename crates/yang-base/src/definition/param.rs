//! Action Params 的原生定义集合。

use super::{FieldName, FieldRef, IntoFieldSpec, ParamSource, ParamSpec};

/// 有序参数集合；每个参数明确标注 body/query/path/header 来源。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Params(Vec<ParamSpec>);

/// 由 `params!` 生成的强类型输入契约。
pub trait ParamInput {
    /// 返回同一声明生成的静态参数定义。
    fn params() -> Params;

    /// 从传输请求按 ParamSpec 来源一次性构造强类型输入。
    ///
    /// 手写实现默认沿用单 body 反序列化；`params!` 会覆盖本方法并合并
    /// body/query/path/header 后只执行一次结构体反序列化。
    fn decode(request: &mut crate::action::Request) -> Result<Self, crate::error::BaseError>
    where
        Self: serde::de::DeserializeOwned + Sized,
    {
        serde_json::from_value(std::mem::take(&mut request.body)).map_err(|error| {
            crate::error::BaseError::ParamInvalid("input".into(), error.to_string())
        })
    }
}

impl Params {
    /// 创建空参数集合。
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// 从类型化字段 Builder 创建 Action 专属参数定义。
    pub fn param<B>(mut self, name: FieldName, source: ParamSource, builder: B) -> Self
    where
        B: IntoFieldSpec,
    {
        let field = builder.into_field_spec(name.clone());
        self.0.push(ParamSpec::from_spec(name, source, field));
        self
    }

    /// 复用 Module 字段的共享语义；引用在 AppBuilder::build 时解析。
    pub fn from_field(mut self, name: FieldName, source: ParamSource, field: FieldRef) -> Self {
        self.0.push(ParamSpec::new(name, source).from_field(field));
        self
    }

    /// 返回参数定义。
    pub fn as_slice(&self) -> &[ParamSpec] {
        &self.0
    }

    pub(crate) fn into_vec(self) -> Vec<ParamSpec> {
        self.0
    }
}
