# 实施计划：模块表路由系统 (Module Table Routing System)

## 概述

本实施计划基于设计文档和需求文档，将模块表路由系统的开发分为 6 个阶段，每个阶段包含明确的实施任务和测试任务。系统将为 yang-base 库提供完整的数据表管理、查询构建和路由分发能力。

### 技术栈

- **语言**: Rust
- **异步运行时**: Tokio
- **数据库**: MySQL (通过 yang-db)
- **序列化**: serde + serde_json
- **错误处理**: thiserror

### 实施原则

1. **类型安全优先**: 充分利用 Rust 类型系统，编译期捕获错误
2. **测试驱动**: 每个功能模块都包含单元测试和集成测试
3. **增量开发**: 每个阶段都能独立运行和测试
4. **文档完善**: 所有公开 API 都包含中文文档注释

## 任务列表

### 第一阶段：基础设施

本阶段实现表配置系统的核心数据结构，包括 TableConfig、FieldConfig、FieldType 和 Validator。


- [x] 1. 实现 FieldType 枚举和类型验证
  - [x] 1.1 定义 FieldType 枚举
    - 实现基本类型: String, Integer, BigInt, Float, Double, Boolean
    - 实现时间类型: Date, DateTime, Timestamp
    - 实现复杂类型: Json, Text
    - 实现枚举类型: Enum { values: Vec<String> }
    - 实现外键类型: ForeignKey { table, field }
    - _需求: 3.1, 3.2, 3.3, 3.4, 3.5_
  
  - [x] 1.2 实现 FieldType 的 validate 方法
    - 实现 String 类型的长度验证 (max_length)
    - 实现 Integer/BigInt 类型的整数验证
    - 实现 Float/Double 类型的浮点数验证
    - 实现 Boolean 类型的布尔值验证
    - 实现 Enum 类型的枚举值验证
    - 实现 Json 类型的 JSON 格式验证
    - _需求: 3.6, 3.7, 3.8, 3.9_
  
  - [x] 1.3 编写 FieldType 单元测试
    - 测试所有基本类型的验证逻辑
    - 测试边界条件 (空字符串、最大长度、数值范围)
    - 测试错误情况 (类型不匹配、枚举值无效)
    - _需求: 3.6, 3.7, 3.8, 3.9_

- [x] 2. 实现 Validator 验证器系统
  - [x] 2.1 定义 Validator 枚举
    - 实现长度验证器: MinLength, MaxLength
    - 实现数值范围验证器: Min, Max
    - 实现格式验证器: Email, Phone, Url
    - 实现正则表达式验证器: Regex
    - 实现自定义验证器: Custom(fn)
    - _需求: 4.1, 4.2, 4.3, 4.4, 4.5_
  
  - [x] 2.2 实现 Validator 的 validate 方法
    - 实现 MinLength/MaxLength 验证逻辑
    - 实现 Min/Max 数值范围验证逻辑
    - 实现 Email 格式验证 (包含 @ 符号)
    - 实现 Phone 格式验证 (数字和连字符)
    - 实现 Url 格式验证 (http/https 前缀)
    - 实现 Regex 正则表达式匹配
    - _需求: 4.6, 4.7, 4.8, 4.9, 4.10, 4.11_
  
  - [x] 2.3 编写 Validator 单元测试
    - 测试所有验证器的正常情况
    - 测试边界条件
    - 测试错误情况和错误消息
    - _需求: 4.6, 4.7, 4.8, 4.9, 4.10, 4.11_


- [x] 3. 实现 FieldConfig 字段配置
  - [x] 3.1 定义 FieldConfig 结构体
    - 定义字段名、显示名称、字段类型
    - 定义必填标记、默认值
    - 定义验证器列表
    - 定义字段级权限配置
    - 定义关联表信息
    - _需求: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_
  
  - [x] 3.2 实现 FieldConfig 的 validate 方法
    - 实现必填字段检查
    - 实现字段类型验证
    - 实现验证器链式执行
    - 实现错误信息收集和返回
    - _需求: 2.7, 2.8, 2.9, 2.10_
  
  - [x] 3.3 实现 FieldConfig 的构建器方法
    - 实现 new() 构造函数
    - 实现 required() 设置必填
    - 实现 default_value() 设置默认值
    - 实现 validator() 添加验证器
    - 实现 permissions() 设置权限
    - 实现 relation() 设置关联表
    - _需求: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_
  
  - [x] 3.4 编写 FieldConfig 单元测试
    - 测试字段验证的完整流程
    - 测试必填字段验证
    - 测试验证器链式执行
    - 测试错误情况处理
    - _需求: 2.7, 2.8, 2.9, 2.10_

- [x] 4. 实现 TableConfig 表配置
  - [x] 4.1 定义 TableConfig 结构体
    - 定义表名、显示名称、主键字段
    - 定义字段列表 (HashMap<String, FieldConfig>)
    - 定义索引配置 (唯一索引、普通索引)
    - 定义默认排序规则
    - 定义软删除字段
    - 定义时间戳字段配置
    - _需求: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_
  
  - [x] 4.2 实现 TableConfig 的验证方法
    - 实现 validate_field() 检查字段存在性
    - 实现 get_field() 获取字段配置
    - 实现 validate_query() 验证查询参数
    - _需求: 1.7, 1.8_
  
  - [x] 4.3 实现 TableConfig 的构建器方法
    - 实现 new() 构造函数
    - 实现 field() 添加字段
    - 实现 primary_key() 设置主键
    - 实现 unique_index() 添加唯一索引
    - 实现 index() 添加普通索引
    - 实现 default_order() 设置默认排序
    - 实现 soft_delete_field() 设置软删除字段
    - 实现 timestamps() 设置时间戳字段
    - _需求: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_
  
  - [x] 4.4 编写 TableConfig 单元测试
    - 测试字段存在性验证
    - 测试查询参数验证
    - 测试索引配置
    - 测试软删除配置
    - _需求: 1.7, 1.8_

- [x] 5. 实现权限配置结构
  - [x] 5.1 定义 FieldPermissions 结构体
    - 定义可读角色列表 (readable_roles)
    - 定义可写角色列表 (writable_roles)
    - 定义可筛选角色列表 (filterable_roles)
    - 定义可排序角色列表 (sortable_roles)
    - _需求: 11.1, 11.2, 11.3, 11.4_
  
  - [x] 5.2 实现 FieldPermissions 的权限检查方法
    - 实现 can_read() 检查读取权限
    - 实现 can_write() 检查写入权限
    - 实现 can_filter() 检查筛选权限
    - 实现 can_sort() 检查排序权限
    - 实现空列表表示允许所有用户的逻辑
    - _需求: 11.5, 11.6, 11.7, 11.8, 11.9, 11.10, 11.11_
  
  - [x] 5.3 编写 FieldPermissions 单元测试
    - 测试各种权限检查方法
    - 测试空列表的默认行为
    - 测试角色匹配逻辑
    - _需求: 11.5, 11.6, 11.7, 11.8, 11.9, 11.10, 11.11_

- [x] 6. 第一阶段检查点
  - 确保所有测试通过
  - 确保代码通过 cargo clippy 检查
  - 确保代码通过 cargo fmt 检查
  - 如有问题请向用户询问

### 第二阶段：查询系统

本阶段实现基于 yang-db 的类型安全查询构建器，包括 TableQuery、QueryParams 和 CRUD 操作。


- [x] 7. 实现查询参数结构
  - [x] 7.1 定义 WhereCondition 枚举
    - 实现 Eq (等于)
    - 实现 In (包含于列表)
    - 实现 Like (模糊匹配)
    - 实现 Gt/Gte (大于/大于等于)
    - 实现 Lt/Lte (小于/小于等于)
    - 实现 IsNull/IsNotNull (空值判断)
    - _需求: 5.3_
  
  - [x] 7.2 定义 QueryParams 结构体
    - 定义字段选择列表 (fields)
    - 定义 WHERE 条件列表 (where_conditions)
    - 定义排序规则列表 (order_by)
    - 定义分页参数 (page, page_size)
    - _需求: 5.2, 5.3, 5.4, 5.5_
  
  - [x] 7.3 定义 PaginatedResult 结构体
    - 定义数据列表 (data)
    - 定义总记录数 (total)
    - 定义当前页码 (page)
    - 定义每页大小 (page_size)
    - 定义总页数 (total_pages)
    - _需求: 5.7_

- [x] 8. 实现 TableQuery 查询构建器
  - [x] 8.1 定义 TableQuery 结构体
    - 定义表配置引用 (Arc<TableConfig>)
    - 定义用户角色列表 (user_roles)
    - 定义查询参数 (QueryParams)
    - 定义数据库连接池引用
    - _需求: 5.1_
  
  - [x] 8.2 实现 TableQuery 的构造方法
    - 实现 new() 构造函数
    - 验证表配置有效性
    - 初始化查询参数
    - _需求: 5.1_
  
  - [x] 8.3 实现 TableQuery 的查询构建方法
    - 实现 select_fields() 选择字段
    - 实现 where_eq() 添加等于条件
    - 实现 where_in() 添加包含条件
    - 实现 where_like() 添加模糊匹配条件
    - 实现 order_by() 添加排序规则
    - 实现 page() 设置分页参数
    - _需求: 5.2, 5.3, 5.4, 5.5_
  
  - [x] 8.4 实现字段权限验证
    - 在 select_fields() 中检查字段读取权限
    - 在 where_*() 中检查字段筛选权限
    - 在 order_by() 中检查字段排序权限
    - 权限检查失败时返回 FieldPermissionDenied 错误
    - _需求: 5.12, 5.13, 5.14, 5.15, 5.16_
  
  - [x] 8.5 编写 TableQuery 构建器单元测试
    - 测试查询构建方法的链式调用
    - 测试字段权限验证
    - 测试错误情况处理
    - _需求: 5.11, 5.12, 5.13, 5.14, 5.15, 5.16_


- [x] 9. 实现 CRUD 操作
  - [x] 9.1 实现 SELECT 查询操作
    - 实现 select<T>() 方法执行查询
    - 使用 yang-db 构建 SELECT 语句
    - 应用字段选择、WHERE 条件、排序规则
    - 反序列化查询结果为指定类型
    - _需求: 5.6_
  
  - [x] 9.2 实现分页查询操作
    - 实现 paginate<T>() 方法执行分页查询
    - 计算 LIMIT 和 OFFSET
    - 执行 COUNT 查询获取总记录数
    - 执行数据查询
    - 构建 PaginatedResult 返回结果
    - _需求: 5.7_
  
  - [x] 9.3 实现 INSERT 操作
    - 实现 insert() 方法插入数据
    - 验证所有字段值的合法性
    - 检查字段写入权限
    - 使用 yang-db 构建 INSERT 语句
    - 返回影响行数
    - _需求: 5.8, 5.17, 5.18_
  
  - [x] 9.4 实现 UPDATE 操作
    - 实现 update() 方法更新数据
    - 验证所有字段值的合法性
    - 检查字段写入权限
    - 使用 yang-db 构建 UPDATE 语句
    - 应用 WHERE 条件
    - 返回影响行数
    - _需求: 5.9, 5.19, 5.20_
  
  - [x] 9.5 实现 DELETE 操作
    - 实现 delete() 方法删除数据
    - 检查是否配置了软删除字段
    - 如果配置了软删除，执行 UPDATE 设置删除标记
    - 如果未配置软删除，执行物理删除
    - 应用 WHERE 条件
    - 返回影响行数
    - _需求: 5.10, 5.21, 5.22_
  
  - [x] 9.6 编写 CRUD 操作集成测试
    - 测试完整的 CRUD 流程
    - 测试字段验证
    - 测试权限检查
    - 测试软删除逻辑
    - 测试分页查询
    - _需求: 5.6, 5.7, 5.8, 5.9, 5.10, 5.17, 5.18, 5.19, 5.20, 5.21, 5.22_

- [x] 10. 第二阶段检查点
  - 确保所有测试通过
  - 确保查询构建器能正确生成 SQL
  - 确保权限检查正常工作
  - 如有问题请向用户询问

### 第三阶段：路由系统

本阶段实现模块路由器和 Action 系统，包括 ModuleRouter、Action Trait 和内置 CRUD Actions。

- [x] 11. 实现 Action 系统基础
  - [x] 11.1 定义 Request 结构体
    - 定义请求体 (body: serde_json::Value)
    - 定义请求头 (headers: HashMap<String, String>)
    - 定义查询参数 (query: HashMap<String, String>)
    - 定义路径参数 (path_params: HashMap<String, String>)
    - 实现 token() 方法从 Authorization 头提取 Token
    - _需求: 14.1, 14.2, 14.3, 14.4, 14.5_
  
  - [x] 11.2 定义 ApiResponse 结构体
    - 定义状态码 (code: i32)
    - 定义消息 (message: String)
    - 定义数据 (data: Option<serde_json::Value>)
    - 实现 success() 创建成功响应
    - 实现 fail() 创建失败响应
    - 实现 from_error() 从 BaseError 创建响应
    - _需求: 14.6, 14.7, 14.8, 14.9, 14.10, 14.11, 14.12, 14.13_
  
  - [x] 11.3 扩展 BaseError 错误类型
    - 添加 ActionNotFound 错误
    - 添加 PermissionDenied 错误
    - 添加 Unauthorized 错误
    - 添加 FieldNotFound 错误
    - 添加 FieldPermissionDenied 错误
    - 添加 FieldRequired 错误
    - 添加 ValidationFailed 错误
    - 添加 ParamMissing 和 ParamInvalid 错误
    - 添加 RecordNotFound 错误
    - 添加 UserNotFound 和 InvalidPassword 错误
    - 添加 TableConfigNotSet 错误
    - 实现 code() 方法返回错误码
    - _需求: 13.1, 13.2, 13.3, 13.4, 13.5, 13.6, 13.7, 13.8, 13.9, 13.10, 13.11, 13.12_


- [x] 12. 实现 ActionContext 上下文
  - [x] 12.1 定义 ActionContext 结构体
    - 定义请求数据 (request: Request)
    - 定义当前用户 (user: Option<User>)
    - 定义全局工具 (tools: Arc<GlobalTools>)
    - 定义表配置 (table_config: Option<Arc<TableConfig>>)
    - _需求: 8.1, 8.2, 8.3, 8.4_
  
  - [x] 12.2 实现 ActionContext 的辅助方法
    - 实现 param<T>() 获取必填参数
    - 实现 param_optional<T>() 获取可选参数
    - 实现 table_query() 创建 TableQuery
    - 实现 user_roles() 获取用户角色列表
    - 实现 with_user() 设置用户
    - 实现 with_table_config() 设置表配置
    - _需求: 8.5, 8.6, 8.7, 8.8, 8.9, 8.10, 8.11_
  
  - [x] 12.3 编写 ActionContext 单元测试
    - 测试参数获取方法
    - 测试参数类型转换
    - 测试错误情况处理
    - _需求: 8.9, 8.10, 8.11_

- [x] 13. 实现 Action Trait
  - [x] 13.1 定义 Action Trait
    - 定义 execute() 异步方法
    - 定义 name() 方法返回 action 名称
    - 定义 permissions() 方法返回权限列表
    - 定义 display_name() 方法返回显示名称
    - 定义 description() 方法返回描述
    - 定义 params_schema() 方法返回参数结构
    - 定义 is_public() 方法标记是否公开
    - _需求: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 7.9_

- [x] 14. 实现内置 CRUD Actions
  - [x] 14.1 实现 AddAction (新增数据)
    - 从请求中获取 data 参数
    - 使用 ActionContext::table_query() 创建查询构建器
    - 执行 INSERT 操作
    - 返回影响行数
    - _需求: 9.1, 9.2, 9.3_
  
  - [x] 14.2 实现 PutAction (更新数据)
    - 从请求中获取主键值
    - 从请求中获取 data 参数
    - 使用 ActionContext::table_query() 创建查询构建器
    - 添加主键 WHERE 条件
    - 执行 UPDATE 操作
    - 返回影响行数
    - _需求: 9.4, 9.5, 9.6, 9.7_
  
  - [x] 14.3 实现 DelAction (删除数据)
    - 从请求中获取主键值
    - 使用 ActionContext::table_query() 创建查询构建器
    - 添加主键 WHERE 条件
    - 执行 DELETE 操作 (支持软删除)
    - 返回影响行数
    - _需求: 9.8, 9.9, 9.10_
  
  - [x] 14.4 实现 GetAction (获取单条数据)
    - 从请求中获取主键值
    - 使用 ActionContext::table_query() 创建查询构建器
    - 添加主键 WHERE 条件
    - 执行 SELECT 操作
    - 如果记录不存在，返回 RecordNotFound 错误
    - 返回单条记录
    - _需求: 9.11, 9.12, 9.13_
  
  - [x] 14.5 实现 SelectAction (查询列表)
    - 从请求中解析 QueryParams
    - 使用 ActionContext::table_query() 创建查询构建器
    - 应用字段选择、筛选条件、排序规则
    - 执行分页查询
    - 返回 PaginatedResult
    - _需求: 9.14, 9.15, 9.16, 9.17_
  
  - [x] 14.6 实现 TableAction (获取表元数据)
    - 从 ActionContext 获取表配置
    - 根据用户角色过滤字段权限
    - 返回表的元数据信息 (字段列表、索引、权限等)
    - 标记为公开 action (is_public = true)
    - _需求: 9.18, 9.19, 9.20_
  
  - [x] 14.7 编写内置 Actions 单元测试
    - 测试每个 Action 的正常流程
    - 测试参数验证
    - 测试权限检查
    - 测试错误情况
    - _需求: 9.1-9.20_


- [x] 15. 实现 ModuleRouter 模块路由器
  - [x] 15.1 定义 ModuleRouter 结构体
    - 定义模块名称和显示名称
    - 定义表配置 (Option<Arc<TableConfig>>)
    - 定义 Action 注册表 (HashMap<String, Box<dyn Action>>)
    - 定义默认权限要求
    - _需求: 6.1, 6.2, 6.3_
  
  - [x] 15.2 实现 ModuleRouter 的构建器方法
    - 实现 new() 构造函数
    - 实现 table_config() 设置表配置
    - 实现 register_action() 注册单个 Action
    - 实现 register_builtin_actions() 注册所有内置 CRUD Actions
    - 实现 default_permissions() 设置默认权限
    - _需求: 6.1, 6.2, 6.3, 6.4, 6.5_
  
  - [x] 15.3 实现 ModuleRouter 的分发方法
    - 实现 dispatch() 方法分发请求到对应 Action
    - 根据 action 名称查找 Action
    - 如果 Action 不存在，返回 ActionNotFound 错误
    - 检查用户是否满足默认权限要求
    - 检查用户是否满足 Action 权限要求
    - 如果权限检查失败，返回 PermissionDenied 或 Unauthorized 错误
    - 执行 Action 并返回结果
    - _需求: 6.6, 6.7, 6.8, 6.9, 6.10, 6.11_
  
  - [x] 15.4 编写 ModuleRouter 集成测试
    - 测试 Action 注册和查找
    - 测试 Action 分发流程
    - 测试权限检查
    - 测试错误情况处理
    - _需求: 6.6, 6.7, 6.8, 6.9, 6.10, 6.11_

- [x] 16. 第三阶段检查点
  - 确保所有测试通过
  - 确保 Action 路由正常工作
  - 确保内置 CRUD Actions 功能完整
  - 如有问题请向用户询问

### 第四阶段：权限系统

本阶段实现完整的权限认证系统，包括 User 结构、认证中间件和权限检查流程。

- [x] 17. 实现 User 用户结构
  - [x] 17.1 定义 User 结构体
    - 定义用户 ID、用户名、昵称、邮箱
    - 定义角色列表 (roles: Vec<String>)
    - 定义权限列表 (permissions: Vec<String>)
    - _需求: 10.1, 10.2, 10.3_
  
  - [x] 17.2 实现 User 的权限检查方法
    - 实现 has_permission() 检查是否有指定权限
    - 实现 has_role() 检查是否有指定角色
    - 实现 has_any_role() 检查是否有任一角色
    - _需求: 10.4, 10.5, 10.6_
  
  - [x] 17.3 编写 User 单元测试
    - 测试权限检查方法
    - 测试角色检查方法
    - _需求: 10.4, 10.5, 10.6_

- [x] 18. 实现 Permission 权限结构
  - [x] 18.1 定义 Permission 结构体
    - 定义权限字符串 (如 "user.read", "user.write")
    - 实现 new() 构造函数
    - 实现 PartialEq 用于权限比较
  
  - [x] 18.2 实现权限匹配逻辑
    - 支持精确匹配 (user.read)
    - 支持通配符匹配 (user.*)
    - 支持层级匹配


- [ ] 19. 实现认证中间件
  - [ ] 19.1 定义 AuthMiddleware 结构
    - 定义 TokenManager 引用
    - 定义数据库连接池引用
  
  - [ ] 19.2 实现 authenticate() 方法
    - 从请求头中提取 Token (Authorization: Bearer <token>)
    - 如果 Token 不存在，返回 None (允许匿名访问)
    - 使用 TokenManager 验证 Token
    - 如果 Token 无效，返回 Unauthorized 错误
    - 从 Token 中提取用户 ID
    - 从数据库加载用户信息 (包括角色和权限)
    - 如果用户不存在，返回 UserNotFound 错误
    - 返回 User 对象
    - _需求: 10.7, 10.8, 10.9, 10.10, 10.11, 10.12, 10.13_
  
  - [ ] 19.3 编写认证中间件集成测试
    - 测试有效 Token 的认证流程
    - 测试无效 Token 的错误处理
    - 测试缺失 Token 的匿名访问
    - 测试用户不存在的错误处理
    - _需求: 10.7, 10.8, 10.9, 10.10, 10.11, 10.12, 10.13_

- [x] 20. 集成权限检查到路由系统
  - [x] 20.1 在 ModuleRouter::dispatch() 中集成认证
    - 在分发前调用 AuthMiddleware::authenticate()
    - 将 User 对象设置到 ActionContext
    - 如果认证失败且 Action 需要认证，返回错误
  
  - [x] 20.2 在 TableQuery 中集成字段级权限
    - 在所有查询操作中应用字段权限过滤
    - 确保用户只能访问有权限的字段
  
  - [x] 20.3 编写权限集成测试
    - 测试完整的认证和授权流程
    - 测试字段级权限过滤
    - 测试 Action 级权限检查
    - _需求: 10.1-10.13, 11.1-11.11_

- [x] 21. 第四阶段检查点
  - 确保所有测试通过
  - 确保认证流程正常工作
  - 确保权限检查覆盖所有层级
  - 如有问题请向用户询问

### 第五阶段：全局工具

本阶段实现全局工具系统，包括 GlobalTools 结构和工具注册机制。

- [x] 22. 实现 GlobalTools 全局工具系统
  - [x] 22.1 定义 GlobalTools 结构体
    - 定义 TokenManager 引用
    - 定义自定义工具注册表 (HashMap<String, Arc<dyn Any + Send + Sync>>)
    - _需求: 12.1_
  
  - [x] 22.2 实现 GlobalTools 的构造方法
    - 实现 new() 构造函数，接收 TokenManager
    - 初始化工具注册表
    - _需求: 12.1_
  
  - [x] 22.3 实现工具注册和获取方法
    - 实现 register_tool() 注册自定义工具
    - 实现 get_tool<T>() 获取已注册的工具
    - 使用工具名称作为键
    - 支持类型安全的工具获取
    - 如果工具不存在或类型不匹配，返回 None
    - _需求: 12.2, 12.3, 12.4, 12.5, 12.6, 12.7_
  
  - [x] 22.4 编写 GlobalTools 单元测试
    - 测试工具注册
    - 测试工具获取
    - 测试类型安全
    - 测试工具不存在的情况
    - _需求: 12.2, 12.3, 12.4, 12.5, 12.6, 12.7_

- [-] 23. 实现 Redis 工具集成示例
  - [ ] 23.1 定义 RedisTools 结构体
    - 定义 Redis 连接池
    - 实现 new() 构造函数
  
  - [ ] 23.2 实现 Redis 基本操作
    - 实现 get() 获取值
    - 实现 set() 设置值 (支持过期时间)
    - 实现 del() 删除值
    - 实现 exists() 检查键是否存在
  
  - [ ] 23.3 注册 Redis 工具到 GlobalTools
    - 在应用初始化时创建 RedisTools
    - 使用 GlobalTools::register_tool() 注册
    - 在 Action 中通过 ActionContext 访问


- [ ] 24. 实现消息队列工具集成示例
  - [ ] 24.1 定义 MessageQueueTools 结构体
    - 定义消息队列连接
    - 实现 new() 构造函数
  
  - [ ] 24.2 实现消息队列基本操作
    - 实现 publish() 发布消息
    - 实现 subscribe() 订阅消息
    - 实现 consume() 消费消息
  
  - [ ] 24.3 注册消息队列工具到 GlobalTools
    - 在应用初始化时创建 MessageQueueTools
    - 使用 GlobalTools::register_tool() 注册
    - 在 Action 中通过 ActionContext 访问

- [x] 25. 第五阶段检查点
  - 确保所有测试通过
  - 确保工具注册和获取机制正常工作
  - 确保 Redis 和消息队列工具可用
  - 如有问题请向用户询问

### 第六阶段：HTTP 集成与完整示例

本阶段实现 HTTP 路由处理和完整的应用示例。

- [ ] 26. 实现 HTTP 路由处理
  - [ ] 26.1 定义 HTTP 路由结构
    - 定义路由路径格式: /{plugin}/{module}/{action}
    - 定义路由参数提取逻辑
  
  - [ ] 26.2 实现 HTTP 请求处理器
    - 从 HTTP 请求构建 Request 对象
    - 提取请求体、请求头、查询参数、路径参数
    - 调用 ModuleRouter::dispatch() 分发请求
    - 将 ApiResponse 转换为 HTTP 响应
  
  - [ ] 26.3 实现错误处理中间件
    - 捕获所有 BaseError 错误
    - 转换为标准的 ApiResponse
    - 设置正确的 HTTP 状态码
    - 记录错误日志

- [ ] 27. 实现完整的应用示例
  - [ ] 27.1 创建示例 Plugin
    - 定义 UserPlugin 插件
    - 创建 user 模块的 TableConfig
    - 注册内置 CRUD Actions
    - 注册自定义 Actions (如 login, logout)
  
  - [ ] 27.2 创建应用初始化代码
    - 初始化数据库连接池
    - 初始化 GlobalTools
    - 注册 Redis 工具
    - 创建 TokenManager
    - 注册所有 Plugins
  
  - [ ] 27.3 创建 HTTP 服务器
    - 使用 actix-web 创建 HTTP 服务器
    - 注册路由处理器
    - 注册认证中间件
    - 注册错误处理中间件
    - 启动服务器
  
  - [ ] 27.4 编写使用文档
    - 编写 API 使用示例
    - 编写自定义 Action 开发指南
    - 编写自定义工具集成指南
    - 编写部署指南

- [ ] 28. 编写端到端测试
  - [ ] 28.1 测试完整的 CRUD 流程
    - 测试用户注册和登录
    - 测试数据的增删改查
    - 测试权限控制
    - 测试错误处理
  
  - [ ] 28.2 测试并发场景
    - 测试多用户并发访问
    - 测试数据库连接池
    - 测试缓存一致性
  
  - [ ] 28.3 测试性能指标
    - 测试查询响应时间
    - 测试 CRUD 操作响应时间
    - 测试并发处理能力
    - _需求: 15.6, 15.7_

- [ ] 29. 编写文档和示例
  - [ ] 29.1 编写 API 文档
    - 文档化所有公开结构体和方法
    - 添加使用示例
    - 添加最佳实践建议
    - _需求: 17.3, 17.4, 17.6_
  
  - [ ] 29.2 编写扩展指南
    - 编写自定义 Action 开发指南
    - 编写自定义字段类型开发指南
    - 编写自定义验证器开发指南
    - 编写自定义工具集成指南
    - _需求: 18.1, 18.2, 18.3, 18.4, 18.6_
  
  - [ ] 29.3 编写迁移指南
    - 编写从 scs-api 迁移的指南
    - 对比新旧系统的差异
    - 提供迁移示例代码

- [x] 30. 第六阶段检查点
  - 确保所有测试通过
  - 确保文档完整
  - 确保示例应用可运行
  - 如有问题请向用户询问

## 注意事项

### 测试策略

- 标记为 * 的任务为可选测试任务，可根据时间安排决定是否实施
- 每个阶段结束时都有检查点，确保增量开发的质量
- 单元测试覆盖核心逻辑，集成测试覆盖完整流程

### 依赖关系

- 第二阶段依赖第一阶段的表配置系统
- 第三阶段依赖第二阶段的查询系统
- 第四阶段依赖第三阶段的路由系统
- 第五阶段可与第四阶段并行开发
- 第六阶段依赖所有前置阶段

### 代码规范

- 所有代码必须通过 cargo clippy 检查
- 所有代码必须通过 cargo fmt 格式化
- 所有公开 API 必须包含中文文档注释
- 所有测试必须放在 __test__ 文件夹中

### 性能要求

- 查询平均响应时间 < 100ms (_需求: 15.6_)
- CRUD 操作平均响应时间 < 50ms (_需求: 15.7_)
- 支持数据库连接池 (_需求: 15.1_)
- 支持查询结果缓存 (_需求: 15.2_)

### 安全要求

- 使用参数化查询防止 SQL 注入 (_需求: 16.1_)
- 对所有用户输入进行验证 (_需求: 16.2_)
- 实现多层次权限控制 (_需求: 16.3, 16.4, 16.5_)
- 限制敏感字段访问 (_需求: 16.6_)
- 记录权限拒绝事件 (_需求: 16.8_)

## 完成标准

当所有任务完成后，系统应该：

1.  提供完整的表配置和查询构建能力
2.  支持类型安全的 Action 路由
3.  实现多层次的权限控制
4.  提供可扩展的全局工具系统
5.  包含完整的文档和示例
6.  通过所有单元测试和集成测试
7.  满足性能和安全要求

---

**文档版本**: 1.0.0  
**创建日期**: 2025-01-XX  
**最后更新**: 2025-01-XX
