use crate::error::BaseError;
use crate::plugin::{Plugin, PluginLifecycleStage, PluginManager, PluginManagerBuilder};
use async_trait::async_trait;
#[cfg(feature = "plugin-schema")]
use serde_json::Value as JsonValue;
use std::sync::Arc;

/// 测试用插件：无依赖
struct PluginA;

#[async_trait]
impl Plugin for PluginA {
    fn name(&self) -> &str {
        "plugin_a"
    }
}

/// 测试用插件：依赖 plugin_a
struct PluginB;

#[async_trait]
impl Plugin for PluginB {
    fn name(&self) -> &str {
        "plugin_b"
    }

    fn dependencies(&self) -> &[&str] {
        &["plugin_a"]
    }
}

/// 测试用插件：依赖 plugin_b
struct PluginC;

#[async_trait]
impl Plugin for PluginC {
    fn name(&self) -> &str {
        "plugin_c"
    }

    fn dependencies(&self) -> &[&str] {
        &["plugin_b"]
    }
}

/// 测试用插件：注册时返回错误
struct PluginFailing;

#[async_trait]
impl Plugin for PluginFailing {
    fn name(&self) -> &str {
        "plugin_failing"
    }

    async fn on_register(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err("注册失败".into())
    }
}

/// 测试用插件：名称带边界空格
struct PluginASpaced;

#[async_trait]
impl Plugin for PluginASpaced {
    fn name(&self) -> &str {
        " plugin_a "
    }
}

/// 测试用插件：空白名称
struct PluginBlankName;

#[async_trait]
impl Plugin for PluginBlankName {
    fn name(&self) -> &str {
        "   "
    }
}

/// 验证需求: P4 - PluginRegistry::get(name) 的结果与构建前注册的插件一一对应
#[tokio::test]
async fn test_registry_get_matches_registered_plugins() {
    let mut builder = PluginManagerBuilder::new();
    builder.register(PluginA).await.unwrap();
    builder.register(PluginB).await.unwrap();
    builder.register(PluginC).await.unwrap();

    let registry = builder.build().expect("构建注册表应成功");

    // 验证每个注册的插件都能通过 get 找到，且名称一致
    let plugin_a = registry.get("plugin_a");
    assert!(plugin_a.is_some(), "应能找到 plugin_a");
    assert_eq!(plugin_a.unwrap().name(), "plugin_a");

    let plugin_b = registry.get("plugin_b");
    assert!(plugin_b.is_some(), "应能找到 plugin_b");
    assert_eq!(plugin_b.unwrap().name(), "plugin_b");

    let plugin_c = registry.get("plugin_c");
    assert!(plugin_c.is_some(), "应能找到 plugin_c");
    assert_eq!(plugin_c.unwrap().name(), "plugin_c");

    // 验证不存在的插件返回 None
    assert!(
        registry.get("nonexistent").is_none(),
        "不存在的插件应返回 None"
    );
}

/// 验证需求: P4 - get_all() 返回缓存结果，多次调用结果一致
#[tokio::test]
async fn test_registry_get_all_returns_cached_result() {
    let mut builder = PluginManagerBuilder::new();
    builder.register(PluginA).await.unwrap();
    builder.register(PluginB).await.unwrap();
    builder.register(PluginC).await.unwrap();

    let registry = builder.build().expect("构建注册表应成功");

    // 多次调用 get_all() 应返回相同的结果（缓存）
    let all_first = registry.get_all();
    let all_second = registry.get_all();

    // 验证两次调用返回相同数量的插件
    assert_eq!(
        all_first.len(),
        all_second.len(),
        "多次调用 get_all() 应返回相同数量的插件"
    );
    assert_eq!(all_first.len(), 3, "应有 3 个插件");

    // 验证两次调用返回相同的插件名称（顺序一致）
    let names_first: Vec<&str> = all_first.iter().map(|p| p.name()).collect();
    let names_second: Vec<&str> = all_second.iter().map(|p| p.name()).collect();
    assert_eq!(
        names_first, names_second,
        "多次调用 get_all() 应返回相同顺序的插件"
    );
}

/// 验证需求: P4 - get_all() 返回的插件按拓扑顺序排列（依赖先于被依赖者）
#[tokio::test]
async fn test_registry_get_all_topological_order() {
    let mut builder = PluginManagerBuilder::new();
    // 故意以非依赖顺序注册
    builder.register(PluginC).await.unwrap();
    builder.register(PluginA).await.unwrap();
    builder.register(PluginB).await.unwrap();

    let registry = builder.build().expect("构建注册表应成功");
    let all_plugins = registry.get_all();

    // 找到各插件在排序结果中的位置
    let pos_a = all_plugins.iter().position(|p| p.name() == "plugin_a");
    let pos_b = all_plugins.iter().position(|p| p.name() == "plugin_b");
    let pos_c = all_plugins.iter().position(|p| p.name() == "plugin_c");

    assert!(
        pos_a.is_some() && pos_b.is_some() && pos_c.is_some(),
        "所有插件应在排序结果中"
    );

    // 验证拓扑顺序：plugin_a 在 plugin_b 之前，plugin_b 在 plugin_c 之前
    assert!(
        pos_a.unwrap() < pos_b.unwrap(),
        "plugin_a（被依赖）应在 plugin_b 之前"
    );
    assert!(
        pos_b.unwrap() < pos_c.unwrap(),
        "plugin_b（被依赖）应在 plugin_c 之前"
    );
}

/// 验证需求: 9.3 - 注册重名插件应返回错误
#[tokio::test]
async fn test_builder_register_duplicate_returns_error() {
    let mut builder = PluginManagerBuilder::new();
    builder.register(PluginA).await.unwrap();

    // 再次注册同名插件应返回错误
    struct PluginADuplicate;
    #[async_trait]
    impl Plugin for PluginADuplicate {
        fn name(&self) -> &str {
            "plugin_a"
        }
    }

    let result = builder.register(PluginADuplicate).await;
    assert!(
        matches!(result, Err(BaseError::PluginAlreadyRegistered(_))),
        "注册重名插件应返回 PluginAlreadyRegistered 错误"
    );
}

#[tokio::test]
async fn test_builder_register_trims_plugin_names() {
    let mut builder = PluginManagerBuilder::new();
    builder
        .register(PluginASpaced)
        .await
        .expect("插件名应在构建期注册时 trim");

    let result = builder.register(PluginA).await;
    assert!(
        matches!(result, Err(BaseError::PluginAlreadyRegistered(name)) if name == "plugin_a"),
        "trim 后的重复插件名应返回 PluginAlreadyRegistered"
    );

    let registry = builder.build().expect("构建注册表应成功");
    assert!(registry.get("plugin_a").is_some());
    assert!(registry.get(" plugin_a ").is_some());
}

#[tokio::test]
async fn test_plugin_manager_register_trims_plugin_names_and_config_keys() {
    let manager = PluginManager::new();
    manager
        .register(PluginASpaced)
        .await
        .expect("插件名应在运行期注册时 trim");

    assert!(manager.get("plugin_a").await.is_some());
    assert!(manager.get(" plugin_a ").await.is_some());

    let result = manager.register(PluginA).await;
    assert!(
        matches!(result, Err(BaseError::PluginAlreadyRegistered(name)) if name == "plugin_a"),
        "trim 后的重复插件名应返回 PluginAlreadyRegistered"
    );

    let config = serde_json::json!({"enabled": true});
    manager
        .load_config(" plugin_a ", config.clone())
        .await
        .expect("配置加载应使用 trim 后插件名");

    assert_eq!(manager.get_config("plugin_a").await, Some(config.clone()));
    assert_eq!(manager.get_config(" plugin_a ").await, Some(config));
}

#[tokio::test]
async fn test_register_rejects_blank_plugin_names() {
    let mut builder = PluginManagerBuilder::new();
    let builder_result = builder.register(PluginBlankName).await;
    assert!(
        matches!(builder_result, Err(BaseError::PluginRegisterFailed(name, message)) if name == "<empty>" && message.contains("插件名称不能为空")),
        "构建期注册空白插件名应返回 PluginRegisterFailed"
    );

    let manager = PluginManager::new();
    let manager_result = manager.register(PluginBlankName).await;
    assert!(
        matches!(manager_result, Err(BaseError::PluginRegisterFailed(name, message)) if name == "<empty>" && message.contains("插件名称不能为空")),
        "运行期注册空白插件名应返回 PluginRegisterFailed"
    );
}

/// 验证需求: 9.3 - 注册回调失败时应返回错误
#[tokio::test]
async fn test_builder_register_callback_failure_returns_error() {
    use std::error::Error;

    let mut builder = PluginManagerBuilder::new();
    let result = builder.register(PluginFailing).await;
    let error = match result {
        Err(error) => error,
        Ok(()) => panic!("注册回调失败时不应成功"),
    };
    assert!(matches!(
        &error,
        BaseError::PluginLifecycleFailed {
            plugin,
            stage: PluginLifecycleStage::Register,
            ..
        } if plugin == "plugin_failing"
    ));
    assert_eq!(error.code(), 100003);
    assert!(error.source().is_some(), "注册回调底层错误链不得丢失");
}

/// 验证需求: 9.4 - build() 消费构建器并返回 PluginRegistry
#[tokio::test]
async fn test_builder_build_produces_registry() {
    let mut builder = PluginManagerBuilder::new();
    builder.register(PluginA).await.unwrap();

    let registry = builder.build().expect("构建注册表应成功");

    // 验证 registry 包含注册的插件
    assert!(
        registry.get("plugin_a").is_some(),
        "registry 应包含已注册的插件"
    );
    assert_eq!(registry.get_all().len(), 1, "registry 应有 1 个插件");
}

/// 验证需求: 9.7 - get() 返回正确的插件引用
#[tokio::test]
async fn test_registry_get_returns_correct_plugin() {
    let mut builder = PluginManagerBuilder::new();
    builder.register(PluginA).await.unwrap();
    builder.register(PluginB).await.unwrap();

    let registry = builder.build().expect("构建注册表应成功");

    // 验证 get() 返回正确的插件
    let plugin = registry.get("plugin_a").unwrap();
    assert_eq!(plugin.name(), "plugin_a", "get() 应返回正确名称的插件");
    assert_eq!(plugin.version(), "0.1.0", "get() 应返回正确版本的插件");
}

/// 验证需求: 9.9 - shutdown() 逆序关闭所有插件
#[tokio::test]
async fn test_registry_shutdown_succeeds() {
    let mut builder = PluginManagerBuilder::new();
    builder.register(PluginA).await.unwrap();
    builder.register(PluginB).await.unwrap();

    let registry = builder.build().expect("构建注册表应成功");

    // 验证 shutdown() 成功执行
    let result = registry.shutdown().await;
    assert!(result.is_ok(), "shutdown() 应成功执行");
}

/// 验证需求: 9.9/TEST-6 - shutdown() 按逆拓扑顺序关闭（依赖者先关）
///
/// 插件依赖关系: PluginA ← PluginB ← PluginC
/// 拓扑排序结果: [plugin_a, plugin_b, plugin_c]（依赖在前）
/// 关闭顺序应为逆序: [plugin_c, plugin_b, plugin_a]（依赖者先关）
#[tokio::test]
async fn test_shutdown_calls_in_reverse_topological_order() {
    use std::sync::Mutex;

    static SHUTDOWN_ORDER: Mutex<Vec<String>> = Mutex::new(Vec::new());
    SHUTDOWN_ORDER.lock().unwrap().clear();

    /// 无依赖插件 A
    struct PluginA;
    #[async_trait]
    impl Plugin for PluginA {
        fn name(&self) -> &str {
            "plugin_a"
        }
        async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            SHUTDOWN_ORDER.lock().unwrap().push("plugin_a".to_string());
            Ok(())
        }
    }

    /// 依赖 plugin_a 的插件 B
    struct PluginB;
    #[async_trait]
    impl Plugin for PluginB {
        fn name(&self) -> &str {
            "plugin_b"
        }
        fn dependencies(&self) -> &[&str] {
            &["plugin_a"]
        }
        async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            SHUTDOWN_ORDER.lock().unwrap().push("plugin_b".to_string());
            Ok(())
        }
    }

    /// 依赖 plugin_b 的插件 C（最顶层依赖者）
    struct PluginC;
    #[async_trait]
    impl Plugin for PluginC {
        fn name(&self) -> &str {
            "plugin_c"
        }
        fn dependencies(&self) -> &[&str] {
            &["plugin_b"]
        }
        async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            SHUTDOWN_ORDER.lock().unwrap().push("plugin_c".to_string());
            Ok(())
        }
    }

    let mut builder = PluginManagerBuilder::new();
    builder.register(PluginA).await.unwrap();
    builder.register(PluginB).await.unwrap();
    builder.register(PluginC).await.unwrap();

    let registry = builder.build().expect("构建注册表应成功");
    let result = registry.shutdown().await;
    assert!(result.is_ok(), "shutdown() 应成功执行");

    let order = SHUTDOWN_ORDER.lock().unwrap().clone();
    assert_eq!(
        order,
        vec!["plugin_c", "plugin_b", "plugin_a"],
        "shutdown 应按逆拓扑顺序关闭：C (依赖B) → B (依赖A) → A"
    );
}

/// 验证需求: 9.1/9.2 - PluginManagerBuilder::new() 创建空构建器
#[tokio::test]
async fn test_builder_new_creates_empty_builder() {
    let builder = PluginManagerBuilder::new();
    let registry = builder.build().expect("空构建器构建应成功");

    // 空构建器构建的 registry 应为空
    assert_eq!(registry.get_all().len(), 0, "空构建器应生成空 registry");
    assert!(
        registry.get("any").is_none(),
        "空 registry 不应包含任何插件"
    );
}

/// 验证需求: 20.1, 20.2, 20.3 - build() 检查依赖完整性
#[tokio::test]
async fn test_build_detects_missing_dependency() {
    let mut builder = PluginManagerBuilder::new();
    // 只注册 plugin_b，但 plugin_b 依赖 plugin_a（未注册）
    builder
        .register(PluginB)
        .await
        .expect("注册 plugin_b 应成功");

    let result = builder.build();
    assert!(
        matches!(result, Err(BaseError::PluginDependencyMissing(_, _))),
        "依赖未注册时应返回 PluginDependencyMissing 错误"
    );

    // 验证错误信息包含插件名和依赖名
    if let Err(BaseError::PluginDependencyMissing(plugin, dep)) = result {
        assert_eq!(plugin, "plugin_b", "错误应指向 plugin_b");
        assert_eq!(dep, "plugin_a", "缺失的依赖应是 plugin_a");
    }
}

/// 验证需求: 19.1, 19.2, 19.3, 19.4 - build() 检测循环依赖
#[tokio::test]
async fn test_build_detects_circular_dependency() {
    // 定义循环依赖的插件：X 依赖 Y，Y 依赖 X
    struct PluginX;
    #[async_trait]
    impl Plugin for PluginX {
        fn name(&self) -> &str {
            "plugin_x"
        }
        fn dependencies(&self) -> &[&str] {
            &["plugin_y"]
        }
    }

    struct PluginY;
    #[async_trait]
    impl Plugin for PluginY {
        fn name(&self) -> &str {
            "plugin_y"
        }
        fn dependencies(&self) -> &[&str] {
            &["plugin_x"]
        }
    }

    let mut builder = PluginManagerBuilder::new();
    builder
        .register(PluginX)
        .await
        .expect("注册 plugin_x 应成功");
    builder
        .register(PluginY)
        .await
        .expect("注册 plugin_y 应成功");

    let result = builder.build();
    assert!(
        matches!(result, Err(BaseError::PluginCircularDependency(_))),
        "循环依赖时应返回 PluginCircularDependency 错误"
    );

    // 验证错误信息包含未排序节点
    if let Err(BaseError::PluginCircularDependency(msg)) = result {
        assert!(
            msg.contains("plugin_x") || msg.contains("plugin_y"),
            "错误信息应包含循环中的插件名称"
        );
    }
}

/// 验证需求: 7.1, 7.2, 7.3, 7.4 - validate_config 集成 jsonschema
#[cfg(feature = "plugin-schema")]
#[tokio::test]
async fn test_validate_config_with_schema() {
    use serde_json::json;

    // 定义带 Schema 的插件
    struct SchemaPlugin;
    #[async_trait]
    impl Plugin for SchemaPlugin {
        fn name(&self) -> &str {
            "schema_plugin"
        }
        fn config_schema(&self) -> Option<JsonValue> {
            Some(json!({
                "type": "object",
                "properties": {
                    "host": {"type": "string"},
                    "port": {"type": "integer"}
                },
                "required": ["host", "port"]
            }))
        }
    }

    let manager = PluginManager::new();
    manager.register(SchemaPlugin).await.expect("注册应成功");

    // 合法配置应通过验证
    let valid_config = json!({"host": "localhost", "port": 3306});
    let result = manager.load_config("schema_plugin", valid_config).await;
    assert!(result.is_ok(), "合法配置应通过验证");
}

/// 验证需求: 7.1, 7.2 - 配置不符合 Schema 时返回错误
#[cfg(feature = "plugin-schema")]
#[tokio::test]
async fn test_validate_config_invalid_returns_error() {
    use serde_json::json;

    // 定义带 Schema 的插件
    struct StrictPlugin;
    #[async_trait]
    impl Plugin for StrictPlugin {
        fn name(&self) -> &str {
            "strict_plugin"
        }
        fn config_schema(&self) -> Option<JsonValue> {
            Some(json!({
                "type": "object",
                "properties": {
                    "port": {"type": "integer"}
                },
                "required": ["port"]
            }))
        }
    }

    let manager = PluginManager::new();
    manager.register(StrictPlugin).await.expect("注册应成功");

    // 配置缺少必填字段应返回错误
    let invalid_config = json!({"host": "localhost"});
    let result = manager.load_config("strict_plugin", invalid_config).await;
    assert!(
        matches!(result, Err(BaseError::PluginConfigInvalid(_, _))),
        "配置不符合 Schema 应返回 PluginConfigInvalid 错误"
    );
}

// ==================== C6 并发回归：register TOCTOU ====================

/// 并发注册同名插件（TOCTOU 回归网，对应 I11）。
///
/// `register` 的「read 检查 contains_key → on_register → write insert」是分离的
/// 三段锁，存在 check-then-insert 竞态窗口：多个并发注册同名插件时，可能都越过
/// 检查、最终多次 insert（后写覆盖）。本测试**锁定当前契约**：
/// - 进程不 panic、不死锁，map 不被破坏
/// - 并发结束后插件确实可查到且名称正确
/// - 至少有一个 register 调用返回 Ok（拿到锁的胜出者）
///
/// I11 修复（改单把 write 锁 check-and-insert）后，应能进一步断言「恰好一个
/// Ok、其余 PluginAlreadyRegistered」——届时收紧本测试。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_same_name_register_keeps_map_consistent() {
    struct DupPlugin;
    #[async_trait]
    impl Plugin for DupPlugin {
        fn name(&self) -> &str {
            "dup_plugin"
        }
    }

    let manager = Arc::new(PluginManager::new());

    let mut handles = Vec::new();
    for _ in 0..16 {
        let m = Arc::clone(&manager);
        handles.push(tokio::spawn(async move { m.register(DupPlugin).await }));
    }

    let mut ok_count = 0usize;
    for h in handles {
        // 任务本身不应 panic
        let res = h.await.expect("注册任务不应 panic");
        if res.is_ok() {
            ok_count += 1;
        }
    }

    // 当前契约：至少一个成功（窗口竞态下可能 >1）
    assert!(ok_count >= 1, "并发同名注册应至少有一个成功");

    // map 未被破坏：插件可查到且名称正确
    let got = manager.get("dup_plugin").await;
    assert!(got.is_some(), "并发注册后应能查到 dup_plugin");
    assert_eq!(got.unwrap().name(), "dup_plugin");
}

/// 并发注册不同名插件：全部成功，全部可查到，无丢失。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_distinct_register_no_loss() {
    // 用宏生成固定数量的不同名插件，避免运行时动态 name 生命周期问题
    macro_rules! make_plugin {
        ($ty:ident, $name:literal) => {
            struct $ty;
            #[async_trait]
            impl Plugin for $ty {
                fn name(&self) -> &str {
                    $name
                }
            }
        };
    }
    make_plugin!(P0, "cn_p0");
    make_plugin!(P1, "cn_p1");
    make_plugin!(P2, "cn_p2");
    make_plugin!(P3, "cn_p3");
    make_plugin!(P4, "cn_p4");
    make_plugin!(P5, "cn_p5");
    make_plugin!(P6, "cn_p6");
    make_plugin!(P7, "cn_p7");

    let manager = Arc::new(PluginManager::new());
    let m = Arc::clone(&manager);

    // 并发注册 8 个不同名插件
    let (r0, r1, r2, r3, r4, r5, r6, r7) = tokio::join!(
        {
            let m = Arc::clone(&m);
            async move { m.register(P0).await }
        },
        {
            let m = Arc::clone(&m);
            async move { m.register(P1).await }
        },
        {
            let m = Arc::clone(&m);
            async move { m.register(P2).await }
        },
        {
            let m = Arc::clone(&m);
            async move { m.register(P3).await }
        },
        {
            let m = Arc::clone(&m);
            async move { m.register(P4).await }
        },
        {
            let m = Arc::clone(&m);
            async move { m.register(P5).await }
        },
        {
            let m = Arc::clone(&m);
            async move { m.register(P6).await }
        },
        {
            let m = Arc::clone(&m);
            async move { m.register(P7).await }
        },
    );
    for r in [r0, r1, r2, r3, r4, r5, r6, r7] {
        r.expect("不同名插件注册应全部成功");
    }

    for i in 0..8 {
        let name = format!("cn_p{}", i);
        assert!(
            manager.get(&name).await.is_some(),
            "{} 应已注册且可查到",
            name
        );
    }
}
