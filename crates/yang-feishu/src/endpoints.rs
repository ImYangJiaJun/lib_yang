//! 飞书开放平台域名预设。
//!
//! 成功路径不依赖域名：`Endpoints::custom` 允许私有化/代理部署。Lark 国际版的
//! `upload` 域尚未用真实租户验证，文档中按「未验证」处理。

/// 一条基础 URL。保存为 `String` 是为了让宿主可以注入私有化网关。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseUrl(String);

impl BaseUrl {
    /// 由任意可显示的 URL 构造。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 读取原始 URL。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BaseUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// 飞书三套域名：OpenAPI、OAuth 授权页/令牌端点、审批附件上传。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// 开放平台 API 域名（`https://open.feishu.cn`）。
    pub open: BaseUrl,
    /// OAuth 授权页与令牌端点域名（`https://accounts.feishu.cn`）。
    pub accounts: BaseUrl,
    /// 审批文件上传域名（`https://www.feishu.cn`，不是 open 域）。
    pub upload: BaseUrl,
}

impl Endpoints {
    /// 飞书中国站预设。
    pub fn feishu() -> Self {
        Self {
            open: BaseUrl::new("https://open.feishu.cn"),
            accounts: BaseUrl::new("https://accounts.feishu.cn"),
            upload: BaseUrl::new("https://www.feishu.cn"),
        }
    }

    /// Lark 国际站预设。
    ///
    /// `upload` 域未用真实 Lark 租户验证；上线前应使用 [`Endpoints::custom`]
    /// 覆盖为实测地址。
    pub fn lark() -> Self {
        Self {
            open: BaseUrl::new("https://open.larksuite.com"),
            accounts: BaseUrl::new("https://accounts.larksuite.com"),
            upload: BaseUrl::new("https://www.larksuite.com"),
        }
    }

    /// 完全自定义三套域名（私有化、代理或未验证的 Lark 上传域）。
    pub fn custom(
        open: impl Into<String>,
        accounts: impl Into<String>,
        upload: impl Into<String>,
    ) -> Self {
        Self {
            open: BaseUrl::new(open),
            accounts: BaseUrl::new(accounts),
            upload: BaseUrl::new(upload),
        }
    }
}

impl Default for Endpoints {
    fn default() -> Self {
        Self::feishu()
    }
}
