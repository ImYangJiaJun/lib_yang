//! Action Trait 单元测试

use crate::action::{Action, ActionContext, ApiResponse, GlobalTools, Permission, Request};
use crate::error::BaseError;
use crate::token::TokenManager;
use async_trait::async_trait;
use jsonwebtoken::Algorithm;
use serde_json::json;
use std::sync::Arc;

/// 测试用的简单 Action
struct TestAction {
    name: String,
    display_name: String,
    description: String,
    permissions: Vec<Permission>,
    is_public: bool,
}

impl TestAction {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            display_name: String::new(),
            description: String::new(),
            permissions: Vec::new(),
            is_public: false,
        }
    }

    fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = display_name.into();
        self
    }

    fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    fn with_permissions(mut self, permissions: Vec<Permission>) -> Self {
        self.permissions = permissions;
        self
    }

    fn with_public(mut self, is_public: bool) -> Self {
        self.is_public = is_public;
        self
    }
}

#[async_trait]
impl Action for TestAction {
    async fn execute(&self, context: ActionContext) -> Result<ApiResponse, BaseError> {
        // 简单的测试实现：返回请求参数
        let name: String = context.param("name")?;
        Ok(ApiResponse::success(json!({ "name": name }), "执行成功"))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn display_name(&self) -> &str {
        if self.display_name.is_empty() {
            self.name()
        } else {
            &self.display_name
        }
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn permissions(&self) -> &[Permission] {
        &self.permissions
    }

    fn is_public(&self) -> bool {
        self.is_public
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_creation() {
        let permission = Permission::new("user:create");
        assert_eq!(permission.name(), "user:create");
    }

    #[test]
    fn test_permission_equality() {
        let p1 = Permission::new("user:create");
        let p2 = Permission::new("user:create");
        let p3 = Permission::new("user:delete");

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_action_name() {
        let action = TestAction::new("test_action");
        assert_eq!(action.name(), "test_action");
    }

    #[test]
    fn test_action_display_name_default() {
        let action = TestAction::new("test_action");
        // 默认情况下，display_name 返回 name
        assert_eq!(action.display_name(), "test_action");
    }

    #[test]
    fn test_action_display_name_custom() {
        let action = TestAction::new("test_action").with_display_name("测试操作");
        assert_eq!(action.display_name(), "测试操作");
    }

    #[test]
    fn test_action_description_default() {
        let action = TestAction::new("test_action");
        // 默认情况下，description 返回空字符串
        assert_eq!(action.description(), "");
    }

    #[test]
    fn test_action_description_custom() {
        let action = TestAction::new("test_action").with_description("这是一个测试操作");
        assert_eq!(action.description(), "这是一个测试操作");
    }

    #[test]
    fn test_action_permissions_default() {
        let action = TestAction::new("test_action");
        // 默认情况下，permissions 返回空列表
        assert_eq!(action.permissions().len(), 0);
    }

    #[test]
    fn test_action_permissions_custom() {
        let permissions = vec![
            Permission::new("user:create"),
            Permission::new("admin:access"),
        ];
        let action = TestAction::new("test_action").with_permissions(permissions);

        assert_eq!(action.permissions().len(), 2);
        assert_eq!(action.permissions()[0].name(), "user:create");
        assert_eq!(action.permissions()[1].name(), "admin:access");
    }

    #[test]
    fn test_action_is_public_default() {
        let action = TestAction::new("test_action");
        // 默认情况下，is_public 返回 false
        assert!(!action.is_public());
    }

    #[test]
    fn test_action_is_public_custom() {
        let action = TestAction::new("test_action").with_public(true);
        assert!(action.is_public());
    }

    #[tokio::test]
    async fn test_action_execute_success() {
        let action = TestAction::new("test_action");

        let token_manager = TokenManager::new_symmetric(
            "test_secret_key",
            Algorithm::HS256,
            "test_issuer".to_string(),
            "test_audience".to_string(),
            3600,
            86400,
        );
        let request = Request::new(json!({ "name": "Alice" }));
        let tools = Arc::new(GlobalTools::new(token_manager));
        let context = ActionContext::new(request, tools);

        let result = action.execute(context).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.code, 0);
        assert_eq!(response.message, "执行成功");
        assert!(response.data.is_some());

        let data = response.data.unwrap();
        assert_eq!(data["name"], "Alice");
    }

    #[tokio::test]
    async fn test_action_execute_param_missing() {
        let action = TestAction::new("test_action");

        let token_manager = TokenManager::new_symmetric(
            "test_secret_key",
            Algorithm::HS256,
            "test_issuer".to_string(),
            "test_audience".to_string(),
            3600,
            86400,
        );
        // 请求中缺少 name 参数
        let request = Request::new(json!({}));
        let tools = Arc::new(GlobalTools::new(token_manager));
        let context = ActionContext::new(request, tools);

        let result = action.execute(context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        match error {
            BaseError::ParamMissing(param) => {
                assert_eq!(param, "name");
            }
            _ => panic!("期望 ParamMissing 错误，但得到: {:?}", error),
        }
    }

    #[test]
    fn test_action_params_schema_default() {
        let action = TestAction::new("test_action");
        // 默认情况下，params_schema 返回 None
        assert!(action.params_schema().is_none());
    }

    #[test]
    fn test_action_trait_object() {
        // 测试 Action 可以作为 trait object 使用
        let action: Box<dyn Action> = Box::new(TestAction::new("test_action"));
        assert_eq!(action.name(), "test_action");
        assert!(!action.is_public());
    }

    #[test]
    fn test_action_send_sync() {
        // 测试 Action 实现了 Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TestAction>();
    }
}
