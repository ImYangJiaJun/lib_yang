mod catalog_tests;
#[cfg(feature = "mysql")]
mod crud_catalog_tests;
mod module_router_tests;
#[cfg(feature = "openapi")]
mod openapi_tests;
mod request_id_middleware_tests;
mod tracing_span_tests;
#[cfg(all(feature = "token", feature = "openapi"))]
mod vertical_contract_tests;
