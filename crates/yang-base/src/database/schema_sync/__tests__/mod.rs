//! schema 同步单元测试

#[cfg(feature = "mysql")]
mod check_expression_test;
#[cfg(feature = "postgres")]
mod pg_tests;
