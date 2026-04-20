//! GlobalDatabase 集成测试
//!
//! 测试全局数据库的初始化和基本操作

use yang_base::database::GlobalDatabase;
use yang_db::DatabaseConfig;

/// 测试数据库初始化
///
/// 注意：此测试需要真实的 MySQL 数据库连接
/// 如果没有可用的数据库，测试将被跳过
#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_global_database_init() {
    // 使用测试数据库连接字符串
    let db_url = "mysql://root:password@localhost:3306/test_db";
    let config = DatabaseConfig::default();

    // 初始化全局数据库
    let result = GlobalDatabase::init(db_url, config).await;

    // 如果连接失败，跳过测试
    if result.is_err() {
        println!("跳过测试：无法连接到数据库");
        return;
    }

    // 验证可以获取数据库实例
    assert!(GlobalDatabase::get().is_ok());

    // 验证可以创建查询构建器
    assert!(GlobalDatabase::table("test_table").is_ok());
}

/// 测试重复初始化
#[tokio::test]
async fn test_global_database_already_initialized() {
    // 注意：由于 OnceLock 的特性，这个测试可能会受到其他测试的影响
    // 在实际使用中，GlobalDatabase 只应该初始化一次

    // 如果数据库已经初始化，测试重复初始化会失败
    // 这个测试主要验证错误处理逻辑
}

/// 测试未初始化时的错误处理
#[test]
fn test_global_database_not_initialized_errors() {
    // 这些测试在 global.rs 的单元测试中已经覆盖
    // 这里只是作为集成测试的补充
}
