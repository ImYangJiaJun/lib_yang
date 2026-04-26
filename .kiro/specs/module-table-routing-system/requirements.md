# 需求文档：模块表路由系统 (Module Table Routing System)

## 1. 引言

### 1.1 系统概述

模块表路由系统是 yang-base 库的核心扩展,为基于插件的 Rust 后端应用提供完整的数据表管理、查询构建和路由分发能力。该系统参考 scs-api 的三层架构(addon → module → action),但采用更类型安全的设计,充分利用 Rust 的类型系统和 trait 机制。

### 1.2 系统目标

- 提供类型安全的表配置和查询构建机制
- 实现声明式的数据表元数据管理
- 支持灵活的 action 路由和权限控制
- 提供统一的 CRUD 操作接口
- 支持可扩展的全局工具系统

### 1.3 技术栈

- **语言**: Rust
- **异步运行时**: Tokio
- **数据库**: MySQL (通过 yang-db)
- **序列化**: serde + serde_json
- **错误处理**: thiserror

## 2. 术语表

- **Plugin**: 插件,系统的顶层组织单元,包含多个模块
- **ModuleRouter**: 模块路由器,管理单个模块的表配置和 action 路由
- **TableConfig**: 表配置,声明式定义数据表的元数据、字段、索引和权限
- **FieldConfig**: 字段配置,定义单个字段的类型、验证规则和权限
- **Action**: 操作,处理特定业务逻辑的可执行单元
- **ActionContext**: Action 执行上下文,包含请求信息、用户信息和全局工具
- **TableQuery**: 表查询构建器,基于 yang-db 的类型安全查询接口
- **GlobalTools**: 全局工具集合,提供可扩展的工具注册机制
- **Permission**: 权限,定义用户可执行的操作
- **Validator**: 验证器,用于验证字段值的合法性
- **RBAC**: 基于角色的访问控制 (Role-Based Access Control)

## 3. 功能需求

### 需求 1: 表配置系统

**用户故事**: 作为开发者,我希望通过声明式配置定义数据表结构,以便自动生成 CRUD 操作并减少样板代码。

#### 验收标准

1. THE TableConfig SHALL 支持定义表名、显示名称和主键字段
2. THE TableConfig SHALL 支持添加多个字段配置 (FieldConfig)
3. THE TableConfig SHALL 支持定义唯一索引和普通索引
4. THE TableConfig SHALL 支持配置默认排序规则
5. THE TableConfig SHALL 支持配置软删除字段
6. THE TableConfig SHALL 支持配置时间戳字段 (created_at, updated_at, deleted_at)
7. WHEN 验证字段名称时, THE TableConfig SHALL 检查字段是否存在于配置中
8. WHEN 验证查询参数时, THE TableConfig SHALL 验证所有引用的字段是否存在

### 需求 2: 字段配置与验证

**用户故事**: 作为开发者,我希望为每个字段定义类型、验证规则和权限,以便确保数据的完整性和安全性。

#### 验收标准

1. THE FieldConfig SHALL 支持定义字段名、显示名称和字段类型
2. THE FieldConfig SHALL 支持标记字段为必填或可选
3. THE FieldConfig SHALL 支持设置字段默认值
4. THE FieldConfig SHALL 支持添加多个验证器 (Validator)
5. THE FieldConfig SHALL 支持配置字段级权限 (可读、可写、可筛选、可排序)
6. THE FieldConfig SHALL 支持配置关联表信息 (外键关系)
7. WHEN 验证字段值时, THE FieldConfig SHALL 首先检查必填约束
8. WHEN 验证字段值时, THE FieldConfig SHALL 执行字段类型验证
9. WHEN 验证字段值时, THE FieldConfig SHALL 依次执行所有配置的验证器
10. WHEN 字段值为 null 且字段必填时, THE FieldConfig SHALL 返回 FieldRequired 错误

### 需求 3: 字段类型系统

**用户故事**: 作为开发者,我希望系统支持丰富的字段类型,以便准确表达业务数据模型。

#### 验收标准

1. THE FieldType SHALL 支持基本类型: String, Integer, BigInt, Float, Double, Boolean
2. THE FieldType SHALL 支持时间类型: Date, DateTime, Timestamp
3. THE FieldType SHALL 支持复杂类型: Json, Text
4. THE FieldType SHALL 支持枚举类型,并定义可选值列表
5. THE FieldType SHALL 支持外键类型,并关联目标表和字段
6. WHEN 验证 String 类型时, THE FieldType SHALL 检查字符串长度不超过 max_length
7. WHEN 验证 Integer 类型时, THE FieldType SHALL 检查值为有效的 i64 整数
8. WHEN 验证 Enum 类型时, THE FieldType SHALL 检查值在可选值列表中
9. WHEN 验证失败时, THE FieldType SHALL 返回 InvalidFieldType 或 InvalidEnumValue 错误

### 需求 4: 验证器系统

**用户故事**: 作为开发者,我希望为字段添加灵活的验证规则,以便确保数据符合业务要求。

#### 验收标准

1. THE Validator SHALL 支持长度验证: MinLength, MaxLength
2. THE Validator SHALL 支持数值范围验证: Min, Max
3. THE Validator SHALL 支持格式验证: Email, Phone, Url
4. THE Validator SHALL 支持正则表达式验证: Regex
5. THE Validator SHALL 支持自定义验证函数: Custom
6. WHEN 执行 MinLength 验证时, THE Validator SHALL 检查字符串长度不小于指定值
7. WHEN 执行 MaxLength 验证时, THE Validator SHALL 检查字符串长度不大于指定值
8. WHEN 执行 Min 验证时, THE Validator SHALL 检查数值不小于指定值
9. WHEN 执行 Max 验证时, THE Validator SHALL 检查数值不大于指定值
10. WHEN 执行 Email 验证时, THE Validator SHALL 检查字符串包含 @ 符号
11. WHEN 验证失败时, THE Validator SHALL 返回 ValidationFailed 错误并包含详细信息

### 需求 5: 统一查询接口

**用户故事**: 作为开发者,我希望使用类型安全的查询构建器,以便避免 SQL 注入并提高代码可维护性。

#### 验收标准

1. THE TableQuery SHALL 基于 TableConfig 创建查询构建器
2. THE TableQuery SHALL 支持选择指定字段列表
3. THE TableQuery SHALL 支持添加 WHERE 条件: Eq, In, Like 等
4. THE TableQuery SHALL 支持添加排序规则: ORDER BY
5. THE TableQuery SHALL 支持分页查询: LIMIT 和 OFFSET
6. THE TableQuery SHALL 支持执行 SELECT 查询并返回结果列表
7. THE TableQuery SHALL 支持执行分页查询并返回 PaginatedResult
8. THE TableQuery SHALL 支持执行 INSERT 操作并返回影响行数
9. THE TableQuery SHALL 支持执行 UPDATE 操作并返回影响行数
10. THE TableQuery SHALL 支持执行 DELETE 操作并返回影响行数
11. WHEN 选择字段时, THE TableQuery SHALL 验证字段存在性
12. WHEN 选择字段时, THE TableQuery SHALL 检查用户是否有字段读取权限
13. WHEN 添加 WHERE 条件时, THE TableQuery SHALL 验证字段存在性
14. WHEN 添加 WHERE 条件时, THE TableQuery SHALL 检查用户是否有字段筛选权限
15. WHEN 添加排序时, THE TableQuery SHALL 验证字段存在性
16. WHEN 添加排序时, THE TableQuery SHALL 检查用户是否有字段排序权限
17. WHEN 执行 INSERT 时, THE TableQuery SHALL 验证所有字段值的合法性
18. WHEN 执行 INSERT 时, THE TableQuery SHALL 检查用户是否有字段写入权限
19. WHEN 执行 UPDATE 时, THE TableQuery SHALL 验证所有字段值的合法性
20. WHEN 执行 UPDATE 时, THE TableQuery SHALL 检查用户是否有字段写入权限
21. WHEN 配置了软删除字段时, THE TableQuery DELETE 操作 SHALL 执行 UPDATE 而非物理删除
22. WHEN 未配置软删除字段时, THE TableQuery DELETE 操作 SHALL 执行物理删除

### 需求 6: 模块路由系统

**用户故事**: 作为开发者,我希望通过模块路由器管理 action 分发,以便实现清晰的业务逻辑组织。

#### 验收标准

1. THE ModuleRouter SHALL 支持设置模块名称和显示名称
2. THE ModuleRouter SHALL 支持关联 TableConfig
3. THE ModuleRouter SHALL 支持注册多个 Action
4. THE ModuleRouter SHALL 支持注册内置 CRUD Actions: add, put, del, get, select, table
5. THE ModuleRouter SHALL 支持设置默认权限要求
6. WHEN 分发请求时, THE ModuleRouter SHALL 根据 action 名称查找对应的 Action
7. WHEN Action 不存在时, THE ModuleRouter SHALL 返回 ActionNotFound 错误
8. WHEN 分发请求时, THE ModuleRouter SHALL 检查用户是否满足默认权限要求
9. WHEN 分发请求时, THE ModuleRouter SHALL 检查用户是否满足 Action 权限要求
10. WHEN 权限检查失败时, THE ModuleRouter SHALL 返回 PermissionDenied 或 Unauthorized 错误
11. WHEN 权限检查通过时, THE ModuleRouter SHALL 执行 Action 并返回结果



### 需求 7: Action 系统

**用户故事**: 作为开发者,我希望定义和执行各种 action,以便实现灵活的业务逻辑处理。

#### 验收标准

1. THE Action SHALL 实现 execute 方法处理业务逻辑
2. THE Action SHALL 定义 name 方法返回 action 名称
3. THE Action SHALL 定义 permissions 方法返回权限要求列表
4. THE Action SHALL 支持定义 display_name 和 description
5. THE Action SHALL 支持定义 params_schema 描述参数结构
6. THE Action SHALL 支持标记为公开 action (is_public)
7. WHEN 执行 action 时, THE Action SHALL 接收 ActionContext 作为参数
8. WHEN 执行成功时, THE Action SHALL 返回 ApiResponse
9. WHEN 执行失败时, THE Action SHALL 返回 BaseError

### 需求 8: ActionContext 上下文

**用户故事**: 作为开发者,我希望在 action 中访问请求信息、用户信息和全局工具,以便实现完整的业务逻辑。

#### 验收标准

1. THE ActionContext SHALL 包含请求数据 (Request)
2. THE ActionContext SHALL 包含当前用户信息 (Option<User>)
3. THE ActionContext SHALL 包含全局工具 (GlobalTools)
4. THE ActionContext SHALL 包含表配置 (Option<TableConfig>)
5. THE ActionContext SHALL 提供 param 方法获取必填参数
6. THE ActionContext SHALL 提供 param_optional 方法获取可选参数
7. THE ActionContext SHALL 提供 table_query 方法创建 TableQuery
8. THE ActionContext SHALL 提供 user_roles 方法获取用户角色列表
9. WHEN 获取必填参数不存在时, THE ActionContext SHALL 返回 ParamMissing 错误
10. WHEN 获取参数类型不匹配时, THE ActionContext SHALL 返回 ParamInvalid 错误
11. WHEN 创建 TableQuery 但表配置未设置时, THE ActionContext SHALL 返回 TableConfigNotSet 错误

### 需求 9: 内置 CRUD Actions

**用户故事**: 作为开发者,我希望系统提供标准的 CRUD actions,以便快速实现常见的数据操作。

#### 验收标准

1. THE AddAction SHALL 从请求中获取 data 参数
2. THE AddAction SHALL 使用 TableQuery 执行 INSERT 操作
3. THE AddAction SHALL 返回影响行数
4. THE PutAction SHALL 从请求中获取主键值和 data 参数
5. THE PutAction SHALL 使用 TableQuery 执行 UPDATE 操作
6. THE PutAction SHALL 返回影响行数
7. THE DelAction SHALL 从请求中获取主键值
8. THE DelAction SHALL 使用 TableQuery 执行 DELETE 操作
9. THE DelAction SHALL 返回影响行数
10. THE GetAction SHALL 从请求中获取主键值
11. THE GetAction SHALL 使用 TableQuery 执行 SELECT 操作
12. THE GetAction SHALL 返回单条记录
13. WHEN 记录不存在时, THE GetAction SHALL 返回 RecordNotFound 错误
14. THE SelectAction SHALL 从请求中解析 QueryParams
15. THE SelectAction SHALL 应用字段选择、筛选条件和排序规则
16. THE SelectAction SHALL 执行分页查询
17. THE SelectAction SHALL 返回 PaginatedResult
18. THE TableAction SHALL 返回表的元数据信息
19. THE TableAction SHALL 根据用户角色过滤字段权限
20. THE TableAction SHALL 标记为公开 action

### 需求 10: 权限认证系统

**用户故事**: 作为系统管理员,我希望系统提供完善的权限认证机制,以便保护敏感数据和操作。

#### 验收标准

1. THE User SHALL 包含用户 ID、用户名、昵称和邮箱
2. THE User SHALL 包含角色列表 (roles)
3. THE User SHALL 包含权限列表 (permissions)
4. THE User SHALL 提供 has_permission 方法检查是否有指定权限
5. THE User SHALL 提供 has_role 方法检查是否有指定角色
6. THE User SHALL 提供 has_any_role 方法检查是否有任一角色
7. THE AuthMiddleware SHALL 从请求头中提取 Token
8. THE AuthMiddleware SHALL 使用 TokenManager 验证 Token
9. THE AuthMiddleware SHALL 从数据库加载用户信息
10. WHEN Token 不存在时, THE AuthMiddleware SHALL 返回 None
11. WHEN Token 无效时, THE AuthMiddleware SHALL 返回错误
12. WHEN 用户不存在时, THE AuthMiddleware SHALL 返回 UserNotFound 错误
13. WHEN Token 有效时, THE AuthMiddleware SHALL 返回 User 对象

### 需求 11: 字段级权限控制

**用户故事**: 作为系统管理员,我希望控制不同角色对字段的访问权限,以便实现细粒度的数据保护。

#### 验收标准

1. THE FieldPermissions SHALL 定义可读角色列表 (readable_roles)
2. THE FieldPermissions SHALL 定义可写角色列表 (writable_roles)
3. THE FieldPermissions SHALL 定义可筛选角色列表 (filterable_roles)
4. THE FieldPermissions SHALL 定义可排序角色列表 (sortable_roles)
5. THE FieldPermissions SHALL 提供 can_read 方法检查读取权限
6. THE FieldPermissions SHALL 提供 can_write 方法检查写入权限
7. THE FieldPermissions SHALL 提供 can_filter 方法检查筛选权限
8. THE FieldPermissions SHALL 提供 can_sort 方法检查排序权限
9. WHEN 角色列表为空时, THE FieldPermissions SHALL 允许所有用户访问
10. WHEN 用户角色在列表中时, THE FieldPermissions SHALL 允许访问
11. WHEN 用户角色不在列表中时, THE FieldPermissions SHALL 拒绝访问

### 需求 12: 全局工具系统

**用户故事**: 作为开发者,我希望注册和使用全局工具,以便在 action 中访问数据库、缓存等资源。

#### 验收标准

1. THE GlobalTools SHALL 包含 TokenManager
2. THE GlobalTools SHALL 支持注册自定义工具
3. THE GlobalTools SHALL 支持获取已注册的工具
4. WHEN 注册工具时, THE GlobalTools SHALL 使用工具名称作为键
5. WHEN 获取工具时, THE GlobalTools SHALL 根据名称和类型查找
6. WHEN 工具不存在时, THE GlobalTools SHALL 返回 None
7. WHEN 工具类型不匹配时, THE GlobalTools SHALL 返回 None

### 需求 13: 错误处理系统

**用户故事**: 作为开发者,我希望系统提供类型化的错误处理,以便准确识别和处理各种错误情况。

#### 验收标准

1. THE BaseError SHALL 定义 ActionNotFound 错误类型
2. THE BaseError SHALL 定义 PermissionDenied 错误类型
3. THE BaseError SHALL 定义 Unauthorized 错误类型
4. THE BaseError SHALL 定义 FieldNotFound 错误类型
5. THE BaseError SHALL 定义 FieldPermissionDenied 错误类型
6. THE BaseError SHALL 定义 FieldRequired 错误类型
7. THE BaseError SHALL 定义 ValidationFailed 错误类型
8. THE BaseError SHALL 定义 ParamMissing 和 ParamInvalid 错误类型
9. THE BaseError SHALL 定义 RecordNotFound 错误类型
10. THE BaseError SHALL 定义 UserNotFound 和 InvalidPassword 错误类型
11. THE BaseError SHALL 提供 code 方法返回错误码
12. WHEN 创建 ApiResponse 时, THE BaseError SHALL 转换为标准响应格式

### 需求 14: 请求和响应格式

**用户故事**: 作为开发者,我希望系统提供统一的请求和响应格式,以便简化前后端对接。

#### 验收标准

1. THE Request SHALL 包含请求体 (body)
2. THE Request SHALL 包含请求头 (headers)
3. THE Request SHALL 包含查询参数 (query)
4. THE Request SHALL 包含路径参数 (path_params)
5. THE Request SHALL 提供 token 方法从 Authorization 头提取 Token
6. THE ApiResponse SHALL 包含状态码 (code)
7. THE ApiResponse SHALL 包含消息 (message)
8. THE ApiResponse SHALL 包含数据 (data)
9. THE ApiResponse SHALL 提供 success 方法创建成功响应
10. THE ApiResponse SHALL 提供 fail 方法创建失败响应
11. THE ApiResponse SHALL 提供 from_error 方法从 BaseError 创建响应
12. WHEN 创建成功响应时, THE ApiResponse code SHALL 为 0
13. WHEN 创建失败响应时, THE ApiResponse code SHALL 为非零错误码

## 4. 非功能需求

### 需求 15: 性能要求

**用户故事**: 作为系统运维人员,我希望系统具有良好的性能表现,以便支持高并发访问。

#### 验收标准

1. THE System SHALL 使用连接池管理数据库连接
2. THE System SHALL 支持查询结果缓存
3. THE System SHALL 支持字段选择优化,减少数据传输
4. THE System SHALL 支持分页查询,避免一次性加载大量数据
5. THE System SHALL 利用数据库索引优化查询性能
6. WHEN 执行查询时, THE System SHALL 平均响应时间小于 100ms
7. WHEN 执行 CRUD 操作时, THE System SHALL 平均响应时间小于 50ms

### 需求 16: 安全要求

**用户故事**: 作为安全管理员,我希望系统提供完善的安全保护机制,以便防止各种安全威胁。

#### 验收标准

1. THE System SHALL 使用参数化查询防止 SQL 注入
2. THE System SHALL 对所有用户输入进行验证和转义
3. THE System SHALL 实现 Action 级权限检查
4. THE System SHALL 实现字段级权限控制
5. THE System SHALL 支持行级权限 (通过 WHERE 条件)
6. THE System SHALL 对敏感字段 (如密码) 限制读取权限
7. THE System SHALL 使用 HTTPS 传输敏感数据
8. THE System SHALL 记录所有权限拒绝事件

### 需求 17: 可维护性要求

**用户故事**: 作为开发者,我希望系统代码清晰易懂,以便快速定位和修复问题。

#### 验收标准

1. THE System SHALL 使用 Rust 类型系统提供编译期检查
2. THE System SHALL 提供完整的中文文档注释
3. THE System SHALL 遵循 Rust 编码规范
4. THE System SHALL 提供单元测试覆盖核心功能
5. THE System SHALL 提供集成测试覆盖完整流程
6. THE System SHALL 使用 thiserror 提供清晰的错误信息
7. THE System SHALL 使用 log 记录关键操作日志

### 需求 18: 可扩展性要求

**用户故事**: 作为开发者,我希望系统支持灵活的扩展机制,以便满足不同的业务需求。

#### 验收标准

1. THE System SHALL 支持自定义 Action
2. THE System SHALL 支持自定义字段类型
3. THE System SHALL 支持自定义验证器
4. THE System SHALL 支持自定义全局工具
5. THE System SHALL 支持插件机制
6. THE System SHALL 提供清晰的扩展接口文档
7. THE System SHALL 保持向后兼容性

