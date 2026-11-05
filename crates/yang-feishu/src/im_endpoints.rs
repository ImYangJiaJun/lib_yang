//! IM 官方端点薄封装（自动生成，路径以官方文档为准）。

use serde_json::Value;

use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

use crate::im::ImClient;

impl ImClient {
    /// 撤回消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message/delete>
    ///
    /// Scope: `im:message:recall, im:message, im:message:send_as_bot`
    pub async fn recall_message(&self, message_id: &str) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 转发单条消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message/forward>
    ///
    /// Scope: `im:message:send_as_bot, im:message`
    pub async fn forward_message(
        &self,
        message_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}/forward");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 合并转发多条消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message/merge_forward>
    ///
    /// Scope: `im:message:send_as_bot, im:message`
    pub async fn merge_forward_message(
        &self,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/im/v1/messages/merge_forward".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 更新已发送的交互式消息卡片
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message-card/patch>
    ///
    /// Scope: `im:message:update, im:message, im:message:send_as_bot`
    pub async fn update_message_card(&self, message_id: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 批量查询当前用户对消息的已读状态
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=read_status&project=im&resource=message&version=v1>
    ///
    /// Scope: `im:message:get_as_user, im:message, im:message:readonly`
    pub async fn batch_query_message_read_status(
        &self,
        bearer_token: &str,
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/im/v1/messages/read_status".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// 查询消息的已读用户列表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message/read_users>
    ///
    /// Scope: `im:message:basic, im:message:readonly, im:message, im:message:get_as_user`
    pub async fn get_message_read_users(
        &self,
        message_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}/read_users");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 发送应用内加急
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/buzz-messages/urgent_app>
    ///
    /// Scope: `im:message.urgent, im:message.urgent:app_send`
    pub async fn send_urgent_app(
        &self,
        message_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}/urgent_app");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 发送电话加急
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/buzz-messages/urgent_phone>
    ///
    /// Scope: `im:message.urgent:phone, im:message.urgent:phone_send`
    pub async fn send_urgent_phone(
        &self,
        message_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}/urgent_phone");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 发送短信加急
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/buzz-messages/urgent_sms>
    ///
    /// Scope: `im:message.urgent:sms, im:message.urgent:sms_send`
    pub async fn send_urgent_sms(
        &self,
        message_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}/urgent_sms");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 创建群聊
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat/create>
    ///
    /// Scope: `im:chat, im:chat:create, im:chat:create_by_user`
    pub async fn create_chat(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/im/v1/chats".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 获取群信息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat/get-2>
    ///
    /// Scope: `im:chat:readonly, im:chat, im:chat:read`
    pub async fn get_chat(&self, chat_id: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取群分享链接
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat/link>
    ///
    /// Scope: `im:chat:readonly, im:chat, im:chat:read`
    pub async fn get_chat_link(&self, chat_id: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/link");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 更新群信息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat/update-2>
    ///
    /// Scope: `im:chat, im:chat:update`
    pub async fn update_chat(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 获取群成员机器人列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=bots&project=im&resource=chat.members&version=v1>
    ///
    /// Scope: `im:chat:readonly, im:chat, im:chat.group_info:readonly, im:chat.members:read`
    pub async fn list_chat_bots(&self, chat_id: &str) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/members/bots");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 将用户或机器人拉入群聊
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat-member/create>
    ///
    /// Scope: `im:chat, im:chat.members:write_only`
    pub async fn add_chat_members(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/members");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 将用户或机器人移出群聊
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat-member/delete>
    ///
    /// Scope: `im:chat, im:chat.members:write_only`
    pub async fn remove_chat_members(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/members");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Delete, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 获取群成员列表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat-member/get>
    ///
    /// Scope: `im:chat:readonly, im:chat, im:chat.group_info:readonly, im:chat.members:read`
    pub async fn list_chat_members(&self, chat_id: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/members");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 指定群管理员
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat-member/add_managers>
    ///
    /// Scope: `im:chat, im:chat.managers:write_only`
    pub async fn add_chat_managers(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/managers/add_managers");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除群管理员
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat-member/delete_managers>
    ///
    /// Scope: `im:chat, im:chat.managers:write_only`
    pub async fn delete_chat_managers(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/managers/delete_managers");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 批量审批入群申请
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=handle&project=im&resource=chat.membership_application&version=v1>
    ///
    /// Scope: `im:chat.membership_application:write`
    pub async fn handle_chat_join_requests(
        &self,
        bearer_token: &str,
        chat_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/membership_applications/handle");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// 列出群的待审批入群申请
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=list&project=im&resource=chat.membership_application&version=v1>
    ///
    /// Scope: `im:chat.membership_application:read`
    pub async fn list_chat_join_requests(
        &self,
        bearer_token: &str,
        chat_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/membership_applications");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取群成员发言权限
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat/get>
    ///
    /// Scope: `im:chat:readonly, im:chat, im:chat.moderation:read`
    pub async fn get_chat_moderation(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/moderation");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 更新群发言权限
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/group/chat/update>
    ///
    /// Scope: `im:chat, im:chat:moderation:write_only`
    pub async fn update_chat_moderation(
        &self,
        chat_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/moderation");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 清空自己在群内的昵称
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=im&resource=chat.nickname&version=v1>
    ///
    /// Scope: `im:chat, im:chat.nickname:write`
    pub async fn delete_chat_nickname(&self, bearer_token: &str, chat_id: &str) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/nickname");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(bearer_token, Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 获取自己在群内的昵称
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=im&resource=chat.nickname&version=v1>
    ///
    /// Scope: `im:chat, im:chat:read, im:chat:readonly, im:chat.nickname:read`
    pub async fn get_chat_nickname(&self, bearer_token: &str, chat_id: &str) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/nickname");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 设置自己在群内的昵称
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=im&resource=chat.nickname&version=v1>
    ///
    /// Scope: `im:chat, im:chat.nickname:write`
    pub async fn update_chat_nickname(
        &self,
        bearer_token: &str,
        chat_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("chat_id", chat_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/chats/{chat_id}/nickname");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(
                bearer_token,
                Method::Put,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// 批量查询当前用户在群内的个人偏好设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_query&project=im&resource=chat_user_setting&version=v1>
    ///
    /// Scope: `im:chat:read, im:chat:readonly, im:chat, im:chat.user_setting:read`
    pub async fn batch_query_chat_user_settings(
        &self,
        bearer_token: &str,
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/im/v1/chat_user_setting/batch_query".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// 批量更新当前用户在群内的个人偏好设置
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_update&project=im&resource=chat_user_setting&version=v1>
    ///
    /// Scope: `im:chat, im:chat.user_setting:write`
    pub async fn batch_update_chat_user_settings(
        &self,
        bearer_token: &str,
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/im/v1/chat_user_setting/batch_update".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// 获取消息或消息链接中资源文件夹的子文件列表
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=folder&project=im&resource=file&version=v1>
    ///
    /// Scope: `im:message.history:readonly, im:message:readonly, im:message`
    pub async fn list_file_folder(&self, file_key: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("file_key", file_key)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/files/{file_key}/folder");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// Pin 消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/pin/create>
    ///
    /// Scope: `im:message:send_as_bot, im:message, im:message.pins:write_only`
    pub async fn create_pin(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/im/v1/pins".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 移除 Pin 消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/pin/delete>
    ///
    /// Scope: `im:message:send_as_bot, im:message, im:message.pins:write_only`
    pub async fn delete_pin(&self, message_id: &str) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/pins/{message_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 获取群内 Pin 消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/pin/list>
    ///
    /// Scope: `im:message:readonly, im:message, im:message.pins:read`
    pub async fn list_pins(&self, query: &[(&str, &str)]) -> Result<Value> {
        let path = "/open-apis/im/v1/pins".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 批量获取消息表情回复
    ///
    /// 官方文档: <https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/im-v1/message-reaction/batch_query>
    ///
    /// Scope: `im:message:readonly, im:message.reactions:read`
    pub async fn batch_query_reactions(
        &self,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/im/v1/messages/reactions/batch_query".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 添加消息表情回复
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message-reaction/create>
    ///
    /// Scope: `im:message, im:message.reactions:write_only`
    pub async fn create_reaction(&self, message_id: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}/reactions");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除消息表情回复
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message-reaction/delete>
    ///
    /// Scope: `im:message, im:message.reactions:write_only`
    pub async fn delete_reaction(&self, message_id: &str, reaction_id: &str) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("reaction_id", reaction_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}/reactions/{reaction_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 获取消息表情回复
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message-reaction/list>
    ///
    /// Scope: `im:message:readonly, im:message.reactions:read`
    pub async fn list_reactions(&self, message_id: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}/reactions");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 转发话题
    ///
    /// 官方文档: <https://open.feishu.cn/document/im-v1/message/forward-2>
    ///
    /// Scope: `im:message:send_as_bot, im:message`
    pub async fn forward_thread(
        &self,
        thread_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("thread_id", thread_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/threads/{thread_id}/forward");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 创建 Feed 分组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create&project=im&resource=feed_group&version=v1>
    ///
    /// Scope: `im:feed_group_v1:write`
    pub async fn create_feed_group(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/im/v1/groups".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// 更新 Feed 分组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update&project=im&resource=feed_group&version=v1>
    ///
    /// Scope: `im:feed_group_v1:write`
    pub async fn update_feed_group(
        &self,
        bearer_token: &str,
        feed_group_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("feed_group_id", feed_group_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/groups/{feed_group_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json_user(
                bearer_token,
                Method::Put,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// 删除 Feed 分组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=delete&project=im&resource=feed_group&version=v1>
    ///
    /// Scope: `im:feed_group_v1:write`
    pub async fn delete_feed_group(
        &self,
        bearer_token: &str,
        feed_group_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("feed_group_id", feed_group_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/groups/{feed_group_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(bearer_token, Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 批量查询 Feed 分组
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_query&project=im&resource=feed_group&version=v1>
    ///
    /// Scope: `im:feed_group_v1:read`
    pub async fn batch_query_feed_groups(
        &self,
        bearer_token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        let path = "/open-apis/im/v1/groups/batch_query".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                true,
            )
            .await
    }

    /// 批量添加 Feed 分组项
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_add_item&project=im&resource=feed_group&version=v1>
    ///
    /// Scope: `im:feed_group_v1:write`
    pub async fn batch_add_feed_group_item(
        &self,
        bearer_token: &str,
        feed_group_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("feed_group_id", feed_group_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/groups/{feed_group_id}/batch_add_item");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// 批量移除 Feed 分组项
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=batch_remove_item&project=im&resource=feed_group&version=v1>
    ///
    /// Scope: `im:feed_group_v1:write`
    pub async fn batch_remove_feed_group_item(
        &self,
        bearer_token: &str,
        feed_group_id: &str,
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("feed_group_id", feed_group_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/groups/{feed_group_id}/batch_remove_item");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(
                bearer_token,
                Method::Post,
                url,
                query,
                Some(body.clone()),
                20,
                false,
            )
            .await
    }

    /// 发送消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message/create>
    ///
    /// Scope: `im:message, im:message:send_as_bot, im:message:send, im:message.send_as_user`
    pub async fn send_message_raw(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/im/v1/messages".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 回复消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message/reply>
    ///
    /// Scope: `im:message, im:message:send_as_bot, im:message:send, im:message.send_as_user`
    pub async fn reply_message(&self, message_id: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}/reply");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 编辑已发送的文本或富文本消息
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/im-v1/message/update>
    ///
    /// Scope: `im:message, im:message:send_as_bot, im:message:update`
    pub async fn edit_message(&self, message_id: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("message_id", message_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/im/v1/messages/{message_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }
}
