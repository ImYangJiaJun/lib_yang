//! 云文档/云盘官方端点薄封装（自动生成，路径以官方文档为准）。

use serde_json::Value;

use crate::error::Result;
use crate::path::validate_opaque_path_segment;
use crate::transport::Method;

use crate::drive::DriveClient;

impl DriveClient {
    /// 复制云空间中的文件到指定文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/drive-v1/file/copy>
    ///
    /// Scope: `drive:drive, docs:document:copy`
    pub async fn copy_file(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/copy");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 在云空间指定父文件夹下新建文件夹
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/drive-v1/folder/create_folder>
    ///
    /// Scope: `drive:drive, space:folder:create`
    pub async fn create_folder(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/drive/v1/files/create_folder".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 获取指定文件夹下的文件清单
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/drive-v1/folder/list>
    ///
    /// Scope: `drive:drive, drive:drive:readonly, space:document:retrieve`
    pub async fn list_files(&self, query: &[(&str, &str)]) -> Result<Value> {
        let path = "/open-apis/drive/v1/files".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 修改云空间文件的标题
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=patch&project=drive&resource=file&version=v1>
    ///
    /// Scope: `docx:document:write_only, sheets:spreadsheet:write_only, base:app:update, drive:file:upload`
    pub async fn patch_file(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 批量查询文件元数据
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/drive-v1/file/batch_query>
    ///
    /// Scope: `drive:drive, drive:drive.metadata:readonly`
    pub async fn batch_query_metas(&self, query: &[(&str, &str)], body: &Value) -> Result<Value> {
        let path = "/open-apis/drive/v1/metas/batch_query".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 判断当前用户对云文档是否具有指定权限
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/permission/permission-member/auth>
    ///
    /// Scope: `drive:drive, docs:doc, sheets:spreadsheet, bitable:bitable, wiki:wiki, bitable:app, docs:permission.member:auth`
    pub async fn auth_permission(&self, token: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("token", token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{token}/members/auth");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 为云文档添加协作者权限
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/permission/permission-member/create>
    ///
    /// Scope: `drive:drive, drive:file, docs:doc, sheets:spreadsheet, wiki:wiki, bitable:bitable, bitable:app, docs:permission.member:create`
    pub async fn create_permission_member(
        &self,
        token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("token", token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{token}/members");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 转移云文档所有者
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/permission/permission-member/transfer_owner>
    ///
    /// Scope: `drive:drive, docs:doc, sheets:spreadsheet, bitable:bitable, wiki:wiki, bitable:app, docs:permission.member:transfer`
    pub async fn transfer_owner(
        &self,
        token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("token", token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{token}/members/transfer_owner");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 获取云文档的公开权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/permission/permission-public/get>
    ///
    /// Scope: `drive:drive, drive:drive:readonly, drive:file, docs:doc, sheets:spreadsheet, wiki:wiki, bitable:bitable, docs:permission.setting:read`
    pub async fn get_public_permission(
        &self,
        token: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("token", token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{token}/public");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 更新云文档的公开权限设置
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/permission/permission-public/patch>
    ///
    /// Scope: `drive:drive, drive:file, docs:doc, sheets:spreadsheet, wiki:wiki, bitable:bitable, docs:permission.setting:write_only`
    pub async fn patch_public_permission(
        &self,
        token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("token", token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/permissions/{token}/public");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 按评论 ID 批量获取云文档评论
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/CommentAPI/batch_query>
    ///
    /// Scope: `drive:drive, drive:drive:readonly, docs:document.comment:read`
    pub async fn batch_query_comments(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/batch_query");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 添加云文档评论（V2，支持 block 锚点与全局评论）
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=create_v2&project=drive&resource=file.comment&version=v1>
    ///
    /// Scope: `docs:document.comment:create, docs:document.comment:write_only`
    pub async fn create_comment_v2(&self, file_token: &str, body: &Value) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/new_comments");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 分页获取云文档评论列表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/CommentAPI/list>
    ///
    /// Scope: `drive:drive, docs:doc, sheets:spreadsheet, drive:drive:readonly, docs:doc:readonly, sheets:spreadsheet:readonly, docs:document.comment:read`
    pub async fn list_comments(&self, file_token: &str, query: &[(&str, &str)]) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 解决或恢复云文档评论
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/CommentAPI/patch>
    ///
    /// Scope: `drive:drive, docs:doc, sheets:spreadsheet, drive:drive:readonly, docs:doc:readonly, sheets:spreadsheet:readonly, docs:document.comment:write_only, docs:document.comment:update`
    pub async fn patch_comment(
        &self,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Patch, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 回复云文档评论
    ///
    /// 官方文档: <https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/drive-v1/file-comment-reply/create>
    ///
    /// Scope: `drive:drive, docs:doc, sheets:spreadsheet, drive:drive:readonly, docs:doc:readonly, sheets:spreadsheet:readonly, docs:document.comment:create`
    pub async fn create_comment_reply(
        &self,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 删除云文档评论回复
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/CommentAPI/delete>
    ///
    /// Scope: `drive:drive, docs:doc, sheets:spreadsheet, drive:drive:readonly, docs:doc:readonly, sheets:spreadsheet:readonly, docs:document.comment:write_only, docs:document.comment:delete`
    pub async fn delete_comment_reply(
        &self,
        file_token: &str,
        comment_id: &str,
        reply_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("reply_id", reply_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies/{reply_id}"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 分页获取评论回复列表
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/CommentAPI/list-2>
    ///
    /// Scope: `drive:drive, drive:drive:readonly, docs:document.comment:read`
    pub async fn list_comment_replies(
        &self,
        file_token: &str,
        comment_id: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 更新评论回复内容
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/CommentAPI/update>
    ///
    /// Scope: `drive:drive, docs:doc, sheets:spreadsheet, drive:drive:readonly, docs:doc:readonly, sheets:spreadsheet:readonly, docs:document.comment:write_only, docs:document.comment:update`
    pub async fn update_comment_reply(
        &self,
        file_token: &str,
        comment_id: &str,
        reply_id: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("comment_id", comment_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        validate_opaque_path_segment("reply_id", reply_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!(
            "/open-apis/drive/v1/files/{file_token}/comments/{comment_id}/replies/{reply_id}"
        );
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Put, url, query, Some(body.clone()), 20, true)
            .await
    }

    /// 添加或取消评论回复的表情回复
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=update_reaction&project=drive&resource=comment_reaction&version=v2>
    ///
    /// Scope: `docs:document.comment:create, docs:document.comment:write_only`
    pub async fn update_comment_reply_reaction(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/files/{file_token}/comments/reaction");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 获取文件统计信息（阅读人数、阅读次数、点赞数）
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/drive-v1/file/get>
    ///
    /// Scope: `drive:drive, drive:drive:readonly, drive:drive.metadata:readonly`
    pub async fn get_file_statistics(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/statistics");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取文件的历史访问记录
    ///
    /// 官方文档: <https://open.feishu.cn/document/server-docs/docs/drive-v1/file-view_record/list>
    ///
    /// Scope: `contact:user.base:readonly, drive:file:view_record:readonly`
    pub async fn list_file_view_records(
        &self,
        file_token: &str,
        query: &[(&str, &str)],
    ) -> Result<Value> {
        validate_opaque_path_segment("file_token", file_token)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v1/files/{file_token}/view_records");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }

    /// 获取当前用户云空间容量信息
    ///
    /// 官方文档: <https://open.feishu.cn/api-explorer?from=op_doc_tab&apiName=get&project=drive&resource=quota_detail&version=v2>
    ///
    /// Scope: `drive:quota_detail:read_one`
    pub async fn get_quota_details(
        &self,
        bearer_token: &str,
        quota_detail_id: &str,
    ) -> Result<Value> {
        validate_opaque_path_segment("quota_detail_id", quota_detail_id)
            .map_err(|error| crate::error::Error::fatal(error.to_string()))?;
        let path = format!("/open-apis/drive/v2/quota_details/{quota_detail_id}");
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json_user(bearer_token, Method::Get, url, query, None, 20, true)
            .await
    }

    /// 取消用户云文档事件订阅
    ///
    /// 官方文档: <https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/drive-v1/user/remove_subscription>
    ///
    /// Scope: `docs:event:subscribe`
    pub async fn remove_user_subscription(&self, query: &[(&str, &str)]) -> Result<Value> {
        let path = "/open-apis/drive/v1/user/remove_subscription".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Delete, url, query, None, 20, true)
            .await
    }

    /// 订阅用户云文档事件
    ///
    /// 官方文档: <https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/drive-v1/user/subscription>
    ///
    /// Scope: `docs:event:subscribe`
    pub async fn subscribe_user_events(&self, body: &Value) -> Result<Value> {
        let path = "/open-apis/drive/v1/user/subscription".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = Vec::new();
        self.core
            .call_json(Method::Post, url, query, Some(body.clone()), 20, false)
            .await
    }

    /// 查询用户云文档事件订阅状态
    ///
    /// 官方文档: <https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/drive-v1/user/subscription_status>
    ///
    /// Scope: `docs:event:subscribe`
    pub async fn get_user_subscription_status(&self, query: &[(&str, &str)]) -> Result<Value> {
        let path = "/open-apis/drive/v1/user/subscription_status".to_string();
        let url = format!("{}{}", self.core.endpoints().open.as_str(), path);
        let query = query
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self.core
            .call_json(Method::Get, url, query, None, 20, true)
            .await
    }
}
