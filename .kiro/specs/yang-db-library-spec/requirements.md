# 需求文档

## 简介

yang-db 是一个基于 Rust 的 MySQL 数据库操作库，提供类型安全的查询构建器。本文档定义了该库的功能需求，确保 API 设计清晰、易用且安全。

## 术语表

- **Database**: 数据库连接管理器，负责建立和维护数据库连接
- **QueryBuilder**: 查询构建器，提供链式 API 构建 SQL 查询
- **Condition**: 查询条件，表示 WHERE 子句中的条件表达式
- **SqlValue**: SQL 值类型，表示可以在 SQL 中使用的各种数据类型
- **Transaction**: 事务，保证一组数据库操作的原子性
- **FieldType**: 字段类型标记，用于特殊字段类型的序列化和反序列化
- **JoinClause**: JOIN 子句，用于表连接查询
- **OrderClause**: ORDER BY 子句，用于结果排序
- **GroupClause**: GROUP BY 子句，用于结果分组

## 需求

### 需求 1: 数据库连接管理

**用户故事:** 作为开发者，我希望能够安全地建立和管理数据库连接，以便执行数据库操作。

#### 验收标准

1. WHEN 提供有效的数据库配置 THEN Database SHALL 成功建立连接
2. WHEN 提供无效的数据库配置 THEN Database SHALL 返回清晰的连接错误
3. WHEN 数据库连接断开 THEN Database SHALL 返回连接错误并提供重连机制
4. THE Database SHALL 支持连接池管理以提高性能
5. WHEN 应用程序关闭 THEN Database SHALL 正确释放所有连接资源

### 需求 2: 类型安全的查询构建

**用户故事:** 作为开发者，我希望使用链式 API 构建 SQL 查询，以便提高代码可读性和类型安全性。

#### 验收标准

1. THE QueryBuilder SHALL 提供链式方法调用接口
2. WHEN 构建查询 THEN QueryBuilder SHALL 在编译时检查类型错误
3. THE QueryBuilder SHALL 支持 SELECT、INSERT、UPDATE、DELETE 操作
4. WHEN 构建复杂查询 THEN QueryBuilder SHALL 保持方法调用顺序的逻辑性
5. THE QueryBuilder SHALL 生成参数化查询以防止 SQL 注入

### 需求 3: 查询条件构建

**用户故事:** 作为开发者，我希望能够灵活地构建查询条件，以便实现各种查询需求。

#### 验收标准

1. THE Condition SHALL 支持基本比较操作符（=, !=, >, <, >=, <=）
2. THE Condition SHALL 支持 IN 操作符用于多值匹配
3. THE Condition SHALL 支持 BETWEEN 操作符用于范围查询
4. THE Condition SHALL 支持 LIKE 操作符用于模糊匹配
5. THE Condition SHALL 支持 AND 和 OR 逻辑组合
6. WHEN 组合多个条件 THEN Condition SHALL 正确处理操作符优先级
7. THE Condition SHALL 支持 IS NULL 和 IS NOT NULL 判断

### 需求 4: SQL 值类型转换

**用户故事:** 作为开发者，我希望能够安全地在 Rust 类型和 SQL 类型之间转换，以便处理各种数据类型。

#### 验收标准

1. THE SqlValue SHALL 支持 NULL 值表示
2. THE SqlValue SHALL 支持布尔值（Bool）
3. THE SqlValue SHALL 支持整数（Int/i64）
4. THE SqlValue SHALL 支持浮点数（Float/f64）
5. THE SqlValue SHALL 支持字符串（String）
6. THE SqlValue SHALL 支持字节数组（Bytes）
7. THE SqlValue SHALL 支持 JSON 值
8. THE SqlValue SHALL 支持日期时间（DateTime）
9. THE SqlValue SHALL 支持时间戳（Timestamp）
10. WHEN 类型转换失败 THEN SqlValue SHALL 返回清晰的类型转换错误

### 需求 5: 事务管理

**用户故事:** 作为开发者，我希望能够使用事务来保证多个操作的原子性，以便维护数据一致性。

#### 验收标准

1. THE Database SHALL 提供开始事务的方法
2. WHEN 事务中的所有操作成功 THEN Transaction SHALL 提交所有更改
3. WHEN 事务中任何操作失败 THEN Transaction SHALL 回滚所有更改
4. THE Transaction SHALL 支持嵌套事务或保存点
5. WHEN 事务超时 THEN Transaction SHALL 自动回滚并返回错误

### 需求 6: 特殊字段类型处理

**用户故事:** 作为开发者，我希望能够标记和处理特殊字段类型，以便正确序列化和反序列化数据。

#### 验收标准

1. THE FieldType SHALL 支持 JSON 字段类型标记
2. THE FieldType SHALL 支持 DateTime 字段类型标记
3. THE FieldType SHALL 支持 Timestamp 字段类型标记
4. THE FieldType SHALL 支持 Decimal 字段类型标记
5. THE FieldType SHALL 支持 Blob 字段类型标记
6. THE FieldType SHALL 支持 Text 字段类型标记
7. WHEN 序列化特殊字段 THEN FieldType SHALL 使用正确的格式
8. WHEN 反序列化特殊字段 THEN FieldType SHALL 正确解析数据

### 需求 7: JOIN 查询支持

**用户故事:** 作为开发者，我希望能够执行表连接查询，以便从多个表中获取关联数据。

#### 验收标准

1. THE QueryBuilder SHALL 支持 INNER JOIN
2. THE QueryBuilder SHALL 支持 LEFT JOIN
3. THE QueryBuilder SHALL 支持 RIGHT JOIN
4. WHEN 执行 JOIN 查询 THEN QueryBuilder SHALL 正确生成 ON 子句
5. THE QueryBuilder SHALL 支持多表 JOIN
6. WHEN JOIN 条件无效 THEN QueryBuilder SHALL 返回语法错误

### 需求 8: 排序和分组

**用户故事:** 作为开发者，我希望能够对查询结果进行排序和分组，以便按需组织数据。

#### 验收标准

1. THE QueryBuilder SHALL 支持 ORDER BY 子句
2. THE QueryBuilder SHALL 支持升序（ASC）和降序（DESC）排序
3. THE QueryBuilder SHALL 支持多字段排序
4. THE QueryBuilder SHALL 支持 GROUP BY 子句
5. WHEN 使用 GROUP BY THEN QueryBuilder SHALL 支持 HAVING 子句
6. WHEN 排序字段不存在 THEN QueryBuilder SHALL 返回错误

### 需求 9: 分页支持

**用户故事:** 作为开发者，我希望能够对查询结果进行分页，以便处理大量数据。

#### 验收标准

1. THE QueryBuilder SHALL 支持 LIMIT 子句限制返回行数
2. THE QueryBuilder SHALL 支持 OFFSET 子句跳过指定行数
3. WHEN LIMIT 或 OFFSET 为负数 THEN QueryBuilder SHALL 返回参数错误
4. THE QueryBuilder SHALL 提供便捷的分页方法（page, per_page）

### 需求 10: CRUD 操作

**用户故事:** 作为开发者，我希望能够执行基本的 CRUD 操作，以便管理数据库数据。

#### 验收标准

1. THE QueryBuilder SHALL 支持 INSERT 操作插入单条记录
2. THE QueryBuilder SHALL 支持批量 INSERT 操作
3. THE QueryBuilder SHALL 支持 SELECT 操作查询记录
4. THE QueryBuilder SHALL 支持 UPDATE 操作更新记录
5. THE QueryBuilder SHALL 支持 DELETE 操作删除记录
6. WHEN 执行 UPDATE 或 DELETE 且缺少 WHERE 条件 THEN QueryBuilder SHALL 返回安全错误
7. WHEN INSERT 违反唯一约束 THEN QueryBuilder SHALL 返回约束错误
8. THE QueryBuilder SHALL 返回受影响的行数

### 需求 11: 原生 SQL 执行

**用户故事:** 作为开发者，我希望能够执行原生 SQL 语句，以便处理复杂或特殊的查询需求。

#### 验收标准

1. THE Database SHALL 提供执行原生 SQL 查询的方法
2. THE Database SHALL 支持原生 SQL 的参数绑定
3. WHEN 执行原生 SQL THEN Database SHALL 返回查询结果或受影响行数
4. WHEN 原生 SQL 语法错误 THEN Database SHALL 返回清晰的语法错误

### 需求 12: 错误处理

**用户故事:** 作为开发者，我希望能够获得清晰的错误信息，以便快速定位和解决问题。

#### 验收标准

1. THE DbError SHALL 提供统一的错误类型
2. THE DbError SHALL 包含连接错误（ConnectionError）
3. THE DbError SHALL 包含查询错误（QueryError）
4. THE DbError SHALL 包含 SQL 语法错误（SqlSyntaxError）
5. THE DbError SHALL 包含约束错误（ConstraintError）
6. THE DbError SHALL 包含类型转换错误（TypeConversionError）
7. THE DbError SHALL 包含序列化错误（SerializationError）
8. THE DbError SHALL 包含反序列化错误（DeserializationError）
9. THE DbError SHALL 包含事务错误（TransactionError）
10. THE DbError SHALL 包含表不存在错误（TableNotFound）
11. THE DbError SHALL 包含缺少 WHERE 条件错误（MissingWhereClause）
12. WHEN 发生错误 THEN DbError SHALL 提供中文错误消息
13. WHEN 发生错误 THEN DbError SHALL 包含足够的上下文信息用于调试

### 需求 13: 异步操作支持

**用户故事:** 作为开发者，我希望所有数据库操作都是异步的，以便提高应用程序性能。

#### 验收标准

1. THE Database SHALL 使用 async/await 模式
2. THE QueryBuilder SHALL 提供异步执行方法
3. THE Transaction SHALL 支持异步提交和回滚
4. WHEN 执行异步操作 THEN 系统 SHALL 不阻塞其他任务
5. THE Database SHALL 基于 tokio 运行时

### 需求 14: 查询结果反序列化

**用户故事:** 作为开发者，我希望能够将查询结果自动反序列化为 Rust 结构体，以便类型安全地使用数据。

#### 验收标准

1. THE QueryBuilder SHALL 支持将查询结果反序列化为自定义结构体
2. WHEN 结构体字段与数据库列不匹配 THEN QueryBuilder SHALL 返回反序列化错误
3. THE QueryBuilder SHALL 支持 Option 类型字段处理 NULL 值
4. THE QueryBuilder SHALL 支持嵌套结构体的反序列化
5. WHEN 反序列化 JSON 字段 THEN QueryBuilder SHALL 自动解析 JSON 字符串

### 需求 15: 批量操作优化

**用户故事:** 作为开发者，我希望能够高效地执行批量操作，以便提高大数据量处理的性能。

#### 验收标准

1. THE QueryBuilder SHALL 支持批量插入多条记录
2. WHEN 批量插入 THEN QueryBuilder SHALL 使用单个 SQL 语句
3. THE QueryBuilder SHALL 支持批量更新操作
4. WHEN 批量操作失败 THEN QueryBuilder SHALL 提供详细的失败信息
5. THE QueryBuilder SHALL 支持配置批量操作的批次大小

