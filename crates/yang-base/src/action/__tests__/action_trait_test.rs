//! Permission 权限类型单元测试
//!
//! 旧的对象安全 `Action` trait 已随 H-1 类型化重构移除，本文件现仅覆盖
//! [`Permission`](crate::action::Permission) 的语义与 Cow 零拷贝优化。

use crate::action::Permission;

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

    // ===== Permission Cow 优化相关测试（需求 10）=====

    /// 验证 from_static 创建的 Permission 与 new 创建的 Permission 在 name() 返回值上相同
    #[test]
    fn test_permission_from_static_name_equals_new() {
        // 使用 from_static 创建（零拷贝，Cow::Borrowed）
        let static_perm = Permission::from_static("user:create");
        // 使用 new 创建（堆分配，Cow::Owned）
        let owned_perm = Permission::new("user:create");

        // 两者的 name() 返回值应相同
        assert_eq!(static_perm.name(), owned_perm.name());
        assert_eq!(static_perm.name(), "user:create");
    }

    /// 验证 from_static 创建的 Permission 与 new 创建的 Permission 相等
    #[test]
    fn test_permission_from_static_equals_new() {
        let static_perm = Permission::from_static("admin:access");
        let owned_perm = Permission::new("admin:access");

        // 两者应相等（PartialEq 基于 name 内容比较）
        assert_eq!(static_perm, owned_perm);
    }

    /// 验证 from_static 创建的 Permission 可以正常克隆
    #[test]
    fn test_permission_from_static_clone() {
        let original = Permission::from_static("user:read");
        let cloned = original.clone();

        // 克隆后 name() 应相同
        assert_eq!(original.name(), cloned.name());
        assert_eq!(cloned.name(), "user:read");
    }

    /// 验证 from_static 创建的 Permission 可以用于 Hash 集合
    #[test]
    fn test_permission_from_static_in_hash_set() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        // 混合使用 from_static 和 new
        set.insert(Permission::from_static("user:create"));
        set.insert(Permission::new("user:create"));

        // 相同名称的权限应只存一个（Hash 和 Eq 基于内容）
        assert_eq!(set.len(), 1);
    }

    /// 验证多个不同静态权限的 name() 返回值正确
    #[test]
    fn test_permission_from_static_multiple() {
        let perms = [
            Permission::from_static("user:create"),
            Permission::from_static("user:read"),
            Permission::from_static("user:update"),
            Permission::from_static("user:delete"),
        ];

        assert_eq!(perms[0].name(), "user:create");
        assert_eq!(perms[1].name(), "user:read");
        assert_eq!(perms[2].name(), "user:update");
        assert_eq!(perms[3].name(), "user:delete");
    }
}
