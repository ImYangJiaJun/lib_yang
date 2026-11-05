//! 分页状态机 + 收敛断言。
//!
//! **成功判据不是 `fetched != 0`**：真正的判据是「`page_token` 耗尽且累计行数
//! 不少于首屏 `total`」。三条硬失败（页数超限、`has_more` 缺游标、重复游标）
//! 在迁移时不可「优化」成 break。

use std::collections::BTreeSet;

use crate::error::{Error, Result};

use super::dto::RecordItem;

/// 分页状态机。
#[derive(Debug)]
pub struct PaginationState {
    accumulated: usize,
    first_total: Option<i64>,
    has_more: bool,
    pages: u32,
    max_pages: u32,
    seen_tokens: BTreeSet<String>,
}

impl PaginationState {
    /// 新建状态机；`max_pages` 由调用方按轮次预算给。
    pub fn new(max_pages: u32) -> Self {
        Self {
            accumulated: 0,
            first_total: None,
            has_more: false,
            pages: 0,
            max_pages,
            seen_tokens: BTreeSet::new(),
        }
    }

    /// 累计行数。
    pub fn accumulated(&self) -> usize {
        self.accumulated
    }

    /// 首屏 total。
    pub fn first_total(&self) -> Option<i64> {
        self.first_total
    }

    /// 吞下一屏，返回下一页游标（`None` 表示已耗尽）。
    pub fn accept(
        &mut self,
        has_more: bool,
        page_token: Option<String>,
        total: Option<i64>,
        item_count: usize,
    ) -> Result<Option<String>> {
        self.pages += 1;
        if self.pages > self.max_pages {
            return Err(Error::fatal(format!(
                "翻页数超过上限 {}：疑似 page_token 不推进",
                self.max_pages
            )));
        }
        self.accumulated += item_count;
        self.has_more = has_more;
        if self.first_total.is_none() {
            self.first_total = total;
        }
        if !has_more {
            return Ok(None);
        }
        let cursor = page_token
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                Error::fatal("飞书返回 has_more=true 但没有 page_token，无法继续翻页")
            })?;
        if !self.seen_tokens.insert(cursor.clone()) {
            return Err(Error::fatal(format!(
                "飞书返回了重复的 page_token，分页不会收敛（已翻 {} 页）",
                self.pages
            )));
        }
        Ok(Some(cursor))
    }

    /// 整轮是否收敛。
    pub fn assert_converged(&self) -> Result<()> {
        if self.has_more {
            return Err(Error::fatal("分页未耗尽：最后一屏仍返回 has_more=true"));
        }
        let total = self
            .first_total
            .ok_or_else(|| Error::fatal("首屏未返回 total，无法证明快照完整"))?;
        if total < 0 {
            return Err(Error::fatal(format!("飞书返回的 total 为负数: {total}")));
        }
        if (self.accumulated as i64) < total {
            return Err(Error::fatal(format!(
                "累计行数 {} 少于首屏 total {total}：这是不完整快照",
                self.accumulated
            )));
        }
        Ok(())
    }
}

/// 一轮拉取的结果。
#[derive(Debug, Clone, PartialEq)]
pub struct RecordsSnapshot {
    /// 全部记录。
    pub items: Vec<RecordItem>,
    /// 首屏返回的总记录数。
    pub total: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PaginationState {
        PaginationState::new(40)
    }

    #[test]
    fn two_pages_converge() {
        let mut state = state();
        let next = state
            .accept(true, Some("p1".into()), Some(3), 2)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(next.as_deref(), Some("p1"));
        assert_eq!(
            state
                .accept(false, None, Some(3), 1)
                .unwrap_or_else(|error| panic!("{error}")),
            None
        );
        assert!(state.assert_converged().is_ok());
    }

    #[test]
    fn missing_or_repeated_cursor_is_a_hard_failure() {
        let mut pagination = state();
        assert!(pagination.accept(true, None, Some(2), 1).is_err());

        let mut pagination = state();
        pagination
            .accept(true, Some("p1".into()), Some(2), 1)
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(pagination
            .accept(true, Some("p1".into()), Some(2), 1)
            .is_err());
    }

    #[test]
    fn page_budget_is_enforced() {
        let mut pagination = PaginationState::new(1);
        pagination
            .accept(true, Some("p1".into()), Some(2), 1)
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(pagination
            .accept(true, Some("p2".into()), Some(2), 1)
            .is_err());
    }

    #[test]
    fn convergence_requires_total_and_no_truncation() {
        let mut pagination = state();
        pagination
            .accept(false, None, None, 1)
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(pagination.assert_converged().is_err());

        let mut pagination = state();
        pagination
            .accept(false, None, Some(-1), 1)
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(pagination.assert_converged().is_err());

        let mut pagination = state();
        pagination
            .accept(false, None, Some(5), 4)
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(pagination.assert_converged().is_err());
    }

    #[test]
    fn concurrent_growth_is_tolerated() {
        let mut pagination = state();
        pagination
            .accept(false, None, Some(2), 3)
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(pagination.assert_converged().is_ok());
    }
}
