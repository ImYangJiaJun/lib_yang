//! 表配置系统测试模块

#[cfg(test)]
mod validator_test;

#[cfg(test)]
mod validator_concurrency_test;

#[cfg(test)]
mod definition_test;

#[cfg(test)]
mod schema_validation_test;

#[cfg(test)]
mod query_params_test;

#[cfg(all(test, feature = "mysql"))]
mod table_query_test;
