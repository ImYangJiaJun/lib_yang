//! 内置 CRUD Actions 模块
//!
//! 暂禁 — Task 6 之后用新类型化 builtin 替换。
//! 各 builtin 文件已通过 `#![cfg(any())]` 禁用编译，此处 re-export 同步注释。

// #[cfg(feature = "mysql")]
// mod add;
// #[cfg(feature = "mysql")]
// mod del;
// #[cfg(feature = "mysql")]
// mod get;
// #[cfg(feature = "mysql")]
// mod put;
// #[cfg(feature = "mysql")]
// mod select;
// mod table;

// #[cfg(feature = "mysql")]
// pub use add::AddAction;
// #[cfg(feature = "mysql")]
// pub use del::DelAction;
// #[cfg(feature = "mysql")]
// pub use get::GetAction;
// #[cfg(feature = "mysql")]
// pub use put::PutAction;
// #[cfg(feature = "mysql")]
// pub use select::SelectAction;
// pub use table::TableAction;

#[cfg(test)]
mod __tests__;
