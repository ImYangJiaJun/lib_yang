# yang-db

YANG 数据库操作库，提供类型安全的 MySQL 查询构建器。

## 功能特性

- 链式调用 API，提供流畅的开发体验
- 基于 sqlx 的异步数据库操作
- 类型安全，编译时捕获错误
- 防止 SQL 注入，使用参数化查询
- 支持事务管理
- 完善的错误处理和中文错误消息

## 依赖

- `sqlx`: 异步 SQL 工具包，支持 MySQL
- `tokio`: 异步运行时
- `serde`: 序列化/反序列化
- `thiserror`: 错误处理
- `chrono`: 日期时间处理
- `log`: 日志记录

## 模块结构

- `database`: 数据库连接管理
- `query_builder`: 查询构建器
- `condition`: 查询条件和 SQL 值类型
- `field`: 字段类型和 JOIN/ORDER 子句
- `transaction`: 事务管理
- `error`: 错误类型定义
- `init`: 数据库初始化

## 核心类型

### DbError

统一的错误类型，包含：
- `ConnectionError`: 连接错误
- `QueryError`: 查询错误
- `SqlSyntaxError`: SQL 语法错误
- `ConstraintError`: 约束错误
- `TypeConversionError`: 类型转换错误
- `SerializationError`: 序列化错误
- `DeserializationError`: 反序列化错误
- `TransactionError`: 事务错误
- `TableNotFound`: 表不存在
- `MissingWhereClause`: 缺少 WHERE 条件
- `Unknown`: 未知错误

### SqlValue

SQL 值类型枚举，支持：
- `Null`: NULL 值
- `Bool`: 布尔值
- `Int`: 整数（i64）
- `Float`: 浮点数（f64）
- `String`: 字符串
- `Bytes`: 字节数组
- `Json`: JSON 值
- `DateTime`: 日期时间
- `Timestamp`: 时间戳

### FieldType

字段类型标记：
- `Standard`: 标准类型
- `Json`: JSON 类型
- `DateTime`: DATETIME 类型
- `Timestamp`: TIMESTAMP 类型
- `Decimal`: DECIMAL 类型
- `Blob`: BLOB 类型
- `Text`: TEXT 类型

## 开发状态

当前已完成：
- ✅ 项目结构设置
- ✅ 核心类型定义
- ✅ 依赖配置
- ✅ 基础单元测试

待实现功能：
- 查询构建和执行
- CRUD 操作
- 事务管理
- JOIN 查询
- 聚合函数
- 原生 SQL 支持

## 测试

运行测试：

```bash
cargo test --lib
```

## 代码质量

```bash
# 检查代码
cargo check

# 代码格式化
cargo fmt

# 代码检查
cargo clippy --all-targets -- -D warnings
```
