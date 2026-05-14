// PCG Point 数据结构

use std::collections::BTreeMap;

use crate::model::geometry::{Bounds3, Transform3};

/// UE 侧属性值。
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

/// UE5 PCG 点数据。
#[derive(Debug, Clone)]
pub struct PcgPoint {
    pub transform: Transform3,
    pub bounds: Bounds3,
    pub density: f32,
    pub seed: u64,
    pub attributes: BTreeMap<String, PropertyValue>,
}
