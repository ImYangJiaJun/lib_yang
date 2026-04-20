# 设计文档：yang-base 系统模块管理

## 概述

yang-base 系统模块管理是一个基于 Rust 的模块化架构，为 YANG 项目提供核心的模块注册、管理、数据库初始化、HTTP 通信和 JWT Token 认证能力。该系统参考 scs-api 项目的插件架构设计，通过 Cargo features 实现模块的可选编译和加载，支持模块自定义数据库表结构和初始化逻辑。

**重要说明**：所有数据库相关操作都基于 yang-db 库实现，包括连接管理、查询构建、事务处理等。DatabaseInitializer 和 GlobalDatabase 都是对 yang-db 提供的 Database 类型的封装和扩展。

### 核心功能

1. **模块管理**：提供模块注册、查找、依赖解析和生命周期管理
2. **数据库初始化**：基于 yang-db 库自动执行模块的数据库初始化脚本和迁移
3. **全局数据库访问**：提供线程安全的全局数据库实例（封装 yang-db::Database）
4. **HTTP 客户端**：提供灵活的 HTTP 请求构建和响应处理能力
5. **JWT Token 管理**：提供 Token 生成、验证、解析和刷新机制

### 设计目标

- **模块化**：模块之间松耦合，易于扩展和维护
- **类型安全**：利用 Rust 的类型系统确保编译时安全
- **并发安全**：支持多线程环境下的安全访问
- **易用性**：提供简洁的 API 和清晰的文档
- **可配置性**：通过 Cargo features 灵活控制功能模块

## 架构设计

### 系统架构图

```mermaid
graph TB
    subgraph "应用层"
        APP[应用程序]
    end
    
    subgraph "yang-base 核心层"
        MM[ModuleManager<br/>模块管理器]
        GDB[GlobalDatabase<br/>全局数据库<br/>封装 yang-db::Database]
        HC[HttpClient<br/>HTTP客户端]
        TM[TokenManager<br/>Token管理器]
    end
    
    subgraph "模块层"
        M1[Module 1]
        M2[Module 2]
        M3[Module 3]
    end
    
    subgraph "基础设施层"
        YDB[yang-db 库<br/>Database/QueryBuilder/Transaction]
        DB[(MySQL<br/>数据库)]
        EXT[外部服务<br/>API]
    end
    
    APP --> MM
    APP --> GDB
    APP --> HC
    APP --> TM
    
    MM --> M1
    MM --> M2
    MM --> M3
    
    M1 --> GDB
    M2 --> GDB
    M3 --> GDB
    
    GDB --> YDB
    YDB --> DB
    HC --> EXT
    HC --> TM
    
    style MM fill:#e1f5ff
    style GDB fill:#e1f5ff
    style HC fill:#e1f5ff
    style TM fill:#e1f5ff
    style YDB fill:#fff4e6
```

### 模块划分

系统分为以下核心模块：

1. **module 模块**：模块管理核心
   - `Module` trait：模块接口定义
   - `ModuleManager`：模块注册和管理
   - `ModuleRegistry`：模块注册表

2. **database 模块**：数据库管理（基于 yang-db 库）
   - `GlobalDatabase`：全局数据库实例（封装 yang-db::Database）
   - `DatabaseInitializer`：数据库初始化器（使用 yang-db 方法）
   - `MigrationManager`：迁移管理器

3. **http 模块**：HTTP 客户端
   - `HttpClient`：HTTP 客户端核心
   - `RequestBuilder`：请求构建器
   - `Response`：响应处理
   - `Interceptor`：拦截器机制

4. **token 模块**：JWT Token 管理
   - `TokenManager`：Token 管理器
   - `TokenClaims`：Token 声明
   - `TokenConfig`：Token 配置

5. **error 模块**：错误处理
   - `BaseError`：统一错误类型
   - 各模块特定错误类型

### 数据流图

```mermaid
sequenceDiagram
    participant App as 应用程序
    participant MM as ModuleManager
    participant M as Module
    participant DI as DatabaseInitializer
    participant GDB as GlobalDatabase
    participant YDB as yang-db::Database
    participant DB as MySQL数据库
    
    App->>MM: 注册模块
    MM->>M: 调用 on_register()
    MM->>MM: 检查依赖关系
    MM->>MM: 拓扑排序
    
    App->>DI: 初始化数据库
    DI->>MM: 获取所有模块
    loop 按依赖顺序
        DI->>M: 获取 init_sql()
        DI->>YDB: 调用 Database::execute()
        YDB->>DB: 执行 SQL 脚本
        DI->>M: 调用 on_init()
    end
    
    DI->>GDB: 设置全局数据库实例
    
    App->>GDB: 查询数据
    GDB->>YDB: 调用 Database::query()
    YDB->>DB: 执行查询
    DB-->>YDB: 返回结果
    YDB-->>GDB: 返回结果
    GDB-->>App: 返回数据
```

## 组件和接口

### Module Trait 定义

```rust
use async_trait::async_trait;
use serde_json::Value as JsonValue;

/// 模块接口
///
/// 所有模块必须实现此 trait
#[async_trait]
pub trait Module: Send + Sync {
    /// 获取模块名称
    ///
    /// 模块名称必须唯一，用于标识和查找模块
    fn name(&self) -> &str;
    
    /// 获取模块版本
    ///
    /// 使用语义化版本号，格式：major.minor.patch
    fn version(&self) -> &str {
        "0.1.0"
    }
    
    /// 获取模块依赖列表
    ///
    /// 返回当前模块依赖的其他模块名称列表
    /// 系统会确保依赖模块先于当前模块初始化
    fn dependencies(&self) -> Vec<&str> {
        Vec::new()
    }
    
    /// 获取数据库初始化 SQL 脚本
    ///
    /// 返回创建表的 SQL 语句列表
    /// 建议使用 IF NOT EXISTS 确保幂等性
    fn init_sql(&self) -> Vec<String> {
        Vec::new()
    }
    
    /// 获取数据库迁移脚本
    ///
    /// 返回 (版本号, SQL 脚本) 的列表
    /// 版本号格式：YYYYMMDDHHMMSS
    fn migration_sql(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    
    /// 获取模块配置 Schema
    ///
    /// 返回 JSON Schema 格式的配置定义
    fn config_schema(&self) -> Option<JsonValue> {
        None
    }
    
    /// 模块注册时的回调
    ///
    /// 在模块被注册到 ModuleManager 时调用
    async fn on_register(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    /// 数据库初始化后的回调
    ///
    /// 在模块的数据库表创建完成后调用
    /// 可用于插入初始数据或执行其他初始化逻辑
    async fn on_init(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    
    /// 系统关闭时的回调
    ///
    /// 在系统关闭时调用，用于清理资源
    async fn on_shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
```

### ModuleManager 结构体

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 模块管理器
///
/// 负责模块的注册、查找和生命周期管理
pub struct ModuleManager {
    /// 已注册的模块
    /// Key: 模块名称, Value: 模块实例
    modules: Arc<RwLock<HashMap<String, Arc<dyn Module>>>>,
    
    /// 模块配置
    /// Key: 模块名称, Value: 配置 JSON
    configs: Arc<RwLock<HashMap<String, JsonValue>>>,
}

impl ModuleManager {
    /// 创建新的模块管理器
    pub fn new() -> Self {
        Self {
            modules: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册模块
    ///
    /// # 参数
    /// - module: 模块实例
    ///
    /// # 返回
    /// - Ok(()): 注册成功
    /// - Err(BaseError): 注册失败（如模块名称重复）
    pub async fn register<M: Module + 'static>(
        &self,
        module: M,
    ) -> Result<(), BaseError> {
        let name = module.name().to_string();
        let module = Arc::new(module);
        
        // 检查模块是否已注册
        {
            let modules = self.modules.read().await;
            if modules.contains_key(&name) {
                return Err(BaseError::ModuleAlreadyRegistered(name));
            }
        }
        
        // 调用注册回调
        module.on_register().await
            .map_err(|e| BaseError::ModuleRegisterFailed(name.clone(), e.to_string()))?;
        
        // 注册模块
        {
            let mut modules = self.modules.write().await;
            modules.insert(name.clone(), module);
        }
        
        log::info!("模块已注册: {}", name);
        Ok(())
    }
    
    /// 查找模块
    ///
    /// # 参数
    /// - name: 模块名称
    ///
    /// # 返回
    /// - Some(Arc<dyn Module>): 找到模块
    /// - None: 模块不存在
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Module>> {
        let modules = self.modules.read().await;
        modules.get(name).cloned()
    }
    
    /// 获取所有已注册模块
    ///
    /// # 返回
    /// - 模块列表（按依赖关系排序）
    pub async fn get_all(&self) -> Vec<Arc<dyn Module>> {
        let modules = self.modules.read().await;
        let mut module_list: Vec<_> = modules.values().cloned().collect();
        
        // 按依赖关系排序
        self.topological_sort(&mut module_list);
        
        module_list
    }
    
    /// 加载模块配置
    ///
    /// # 参数
    /// - name: 模块名称
    /// - config: 配置 JSON
    ///
    /// # 返回
    /// - Ok(()): 加载成功
    /// - Err(BaseError): 加载失败（如配置验证失败）
    pub async fn load_config(
        &self,
        name: &str,
        config: JsonValue,
    ) -> Result<(), BaseError> {
        // 获取模块
        let module = self.get(name).await
            .ok_or_else(|| BaseError::ModuleNotFound(name.to_string()))?;
        
        // 验证配置
        if let Some(schema) = module.config_schema() {
            self.validate_config(&config, &schema)?;
        }
        
        // 存储配置
        {
            let mut configs = self.configs.write().await;
            configs.insert(name.to_string(), config);
        }
        
        Ok(())
    }
    
    /// 获取模块配置
    ///
    /// # 参数
    /// - name: 模块名称
    ///
    /// # 返回
    /// - Some(JsonValue): 模块配置
    /// - None: 配置不存在
    pub async fn get_config(&self, name: &str) -> Option<JsonValue> {
        let configs = self.configs.read().await;
        configs.get(name).cloned()
    }
    
    /// 拓扑排序（按依赖关系排序）
    ///
    /// # 参数
    /// - modules: 模块列表
    ///
    /// # 说明
    /// 使用 Kahn 算法进行拓扑排序，确保依赖模块先于当前模块
    fn topological_sort(&self, modules: &mut Vec<Arc<dyn Module>>) {
        // 构建依赖图
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        
        for module in modules.iter() {
            let name = module.name().to_string();
            in_degree.entry(name.clone()).or_insert(0);
            
            for dep in module.dependencies() {
                graph.entry(dep.to_string())
                    .or_insert_with(Vec::new)
                    .push(name.clone());
                *in_degree.entry(name.clone()).or_insert(0) += 1;
            }
        }
        
        // Kahn 算法
        let mut queue: Vec<String> = in_degree.iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(name, _)| name.clone())
            .collect();
        
        let mut sorted = Vec::new();
        
        while let Some(node) = queue.pop() {
            sorted.push(node.clone());
            
            if let Some(neighbors) = graph.get(&node) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(neighbor.clone());
                        }
                    }
                }
            }
        }
        
        // 重新排序模块列表
        modules.sort_by_key(|m| {
            sorted.iter().position(|n| n == m.name()).unwrap_or(usize::MAX)
        });
    }
    
    /// 验证配置
    ///
    /// # 参数
    /// - config: 配置 JSON
    /// - schema: JSON Schema
    ///
    /// # 返回
    /// - Ok(()): 验证通过
    /// - Err(BaseError): 验证失败
    fn validate_config(
        &self,
        _config: &JsonValue,
        _schema: &JsonValue,
    ) -> Result<(), BaseError> {
        // TODO: 实现 JSON Schema 验证
        // 可以使用 jsonschema crate
        Ok(())
    }
    
    /// 关闭所有模块
    ///
    /// 按照依赖关系的逆序调用模块的 on_shutdown 方法
    pub async fn shutdown(&self) -> Result<(), BaseError> {
        let mut modules = self.get_all().await;
        modules.reverse(); // 逆序关闭
        
        for module in modules {
            let name = module.name();
            if let Err(e) = module.on_shutdown().await {
                log::error!("模块 {} 关闭失败: {}", name, e);
                return Err(BaseError::ModuleShutdownFailed(name.to_string(), e.to_string()));
            }
            log::info!("模块已关闭: {}", name);
        }
        
        Ok(())
    }
}

impl Default for ModuleManager {
    fn default() -> Self {
        Self::new()
    }
}
```


### GlobalDatabase 结构体

```rust
use yang_db::{Database, DatabaseConfig, QueryBuilder};
use std::sync::OnceLock;

/// 全局数据库实例
///
/// 提供线程安全的全局数据库访问
/// 这是对 yang-db::Database 的封装，所有数据库操作都通过 yang-db 库实现
static GLOBAL_DB: OnceLock<Database> = OnceLock::new();

/// 全局数据库访问器
///
/// 封装 yang-db::Database，提供全局静态访问接口
pub struct GlobalDatabase;

impl GlobalDatabase {
    /// 初始化全局数据库
    ///
    /// # 参数
    /// - url: 数据库连接字符串
    /// - config: 数据库配置
    ///
    /// # 返回
    /// - Ok(()): 初始化成功
    /// - Err(BaseError): 初始化失败
    ///
    /// # 说明
    /// 使用 yang-db::Database::connect_with_config 创建数据库连接
    pub async fn init(url: &str, config: DatabaseConfig) -> Result<(), BaseError> {
        let db = Database::connect_with_config(url, config).await
            .map_err(|e| BaseError::DatabaseConnectionFailed(e.to_string()))?;
        
        GLOBAL_DB.set(db)
            .map_err(|_| BaseError::DatabaseAlreadyInitialized)?;
        
        log::info!("全局数据库已初始化");
        Ok(())
    }
    
    /// 获取全局数据库实例
    ///
    /// # 返回
    /// - Ok(&Database): yang-db::Database 实例
    /// - Err(BaseError): 数据库未初始化
    pub fn get() -> Result<&'static Database, BaseError> {
        GLOBAL_DB.get()
            .ok_or(BaseError::DatabaseNotInitialized)
    }
    
    /// 选择表，返回查询构建器
    ///
    /// # 参数
    /// - table_name: 表名
    ///
    /// # 返回
    /// - Ok(QueryBuilder): yang-db 查询构建器
    /// - Err(BaseError): 数据库未初始化
    ///
    /// # 说明
    /// 调用 yang-db::Database::table 方法
    pub fn table(table_name: &str) -> Result<QueryBuilder<'static>, BaseError> {
        Ok(Self::get()?.table(table_name))
    }
    
    /// 执行原生 SELECT 查询
    ///
    /// # 参数
    /// - sql: SQL 查询语句
    ///
    /// # 返回
    /// - Ok(Vec<T>): 查询结果
    /// - Err(BaseError): 查询失败
    ///
    /// # 说明
    /// 调用 yang-db::Database::query 方法
    pub async fn query<T>(sql: &str) -> Result<Vec<T>, BaseError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        Self::get()?.query(sql).await
            .map_err(|e| BaseError::DatabaseQueryFailed(e.to_string()))
    }
    
    /// 执行原生 INSERT/UPDATE/DELETE 查询
    ///
    /// # 参数
    /// - sql: SQL 语句
    ///
    /// # 返回
    /// - Ok(u64): 受影响的行数
    /// - Err(BaseError): 执行失败
    ///
    /// # 说明
    /// 调用 yang-db::Database::execute 方法
    pub async fn execute(sql: &str) -> Result<u64, BaseError> {
        Self::get()?.execute(sql).await
            .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))
    }
    
    /// 开始事务
    ///
    /// # 返回
    /// - Ok(Transaction): yang-db 事务对象
    /// - Err(BaseError): 开始事务失败
    ///
    /// # 说明
    /// 调用 yang-db::Database::transaction 方法
    pub async fn transaction() -> Result<yang_db::Transaction, BaseError> {
        Self::get()?.transaction().await
            .map_err(|e| BaseError::DatabaseTransactionFailed(e.to_string()))
    }
}
```

### DatabaseInitializer 结构体

```rust
use yang_db::Database;

/// 数据库初始化器
///
/// 负责执行模块的数据库初始化脚本和迁移
/// 所有数据库操作都通过 yang-db::Database 提供的方法实现
pub struct DatabaseInitializer {
    /// yang-db 数据库实例
    db: Database,
    
    /// 是否启用事务模式
    use_transaction: bool,
}

impl DatabaseInitializer {
    /// 创建新的数据库初始化器
    ///
    /// # 参数
    /// - db: yang-db::Database 实例
    /// - use_transaction: 是否启用事务模式
    pub fn new(db: Database, use_transaction: bool) -> Self {
        Self {
            db,
            use_transaction,
        }
    }
    
    /// 初始化所有模块的数据库
    ///
    /// # 参数
    /// - module_manager: 模块管理器
    ///
    /// # 返回
    /// - Ok(()): 初始化成功
    /// - Err(BaseError): 初始化失败
    ///
    /// # 说明
    /// 使用 yang-db::Database::execute 和 yang-db::Database::transaction 方法
    pub async fn initialize_all(
        &self,
        module_manager: &ModuleManager,
    ) -> Result<(), BaseError> {
        log::info!("开始初始化数据库...");
        
        // 创建迁移记录表（使用 yang-db::Database::execute）
        self.create_migration_table().await?;
        
        // 获取所有模块（已按依赖关系排序）
        let modules = module_manager.get_all().await;
        
        if self.use_transaction {
            // 事务模式：所有模块在一个事务中初始化（使用 yang-db::Transaction）
            self.initialize_with_transaction(&modules).await?;
        } else {
            // 非事务模式：逐个模块初始化（使用 yang-db::Database::execute）
            self.initialize_without_transaction(&modules).await?;
        }
        
        log::info!("数据库初始化完成");
        Ok(())
    }
    
    /// 使用事务模式初始化
    ///
    /// # 说明
    /// 使用 yang-db::Database::transaction 创建事务
    /// 使用 yang-db::Transaction::execute 执行 SQL
    /// 使用 yang-db::Transaction::commit 提交事务
    async fn initialize_with_transaction(
        &self,
        modules: &[Arc<dyn Module>],
    ) -> Result<(), BaseError> {
        let mut tx = self.db.transaction().await
            .map_err(|e| BaseError::DatabaseTransactionFailed(e.to_string()))?;
        
        for module in modules {
            let name = module.name();
            log::info!("初始化模块数据库: {}", name);
            
            // 执行初始化 SQL（使用 yang-db::Transaction::execute）
            for sql in module.init_sql() {
                if let Err(e) = tx.execute(&sql).await {
                    log::error!("模块 {} 初始化失败: {}", name, e);
                    return Err(BaseError::ModuleInitFailed(name.to_string(), e.to_string()));
                }
            }
            
            // 执行迁移
            self.run_migrations_in_tx(&mut tx, module.as_ref()).await?;
            
            // 调用初始化回调
            if let Err(e) = module.on_init().await {
                log::error!("模块 {} 初始化回调失败: {}", name, e);
                return Err(BaseError::ModuleInitFailed(name.to_string(), e.to_string()));
            }
        }
        
        tx.commit().await
            .map_err(|e| BaseError::DatabaseTransactionFailed(e.to_string()))?;
        
        Ok(())
    }
    
    /// 不使用事务模式初始化
    ///
    /// # 说明
    /// 使用 yang-db::Database::execute 执行 SQL
    async fn initialize_without_transaction(
        &self,
        modules: &[Arc<dyn Module>],
    ) -> Result<(), BaseError> {
        for module in modules {
            let name = module.name();
            log::info!("初始化模块数据库: {}", name);
            
            // 执行初始化 SQL（使用 yang-db::Database::execute）
            for sql in module.init_sql() {
                if let Err(e) = self.db.execute(&sql).await {
                    log::error!("模块 {} 初始化失败: {}", name, e);
                    return Err(BaseError::ModuleInitFailed(name.to_string(), e.to_string()));
                }
            }
            
            // 执行迁移
            self.run_migrations(module.as_ref()).await?;
            
            // 调用初始化回调
            if let Err(e) = module.on_init().await {
                log::error!("模块 {} 初始化回调失败: {}", name, e);
                return Err(BaseError::ModuleInitFailed(name.to_string(), e.to_string()));
            }
        }
        
        Ok(())
    }
    
    /// 创建迁移记录表
    ///
    /// # 说明
    /// 使用 yang-db::Database::execute 执行 SQL
    async fn create_migration_table(&self) -> Result<(), BaseError> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                id INT AUTO_INCREMENT PRIMARY KEY,
                module_name VARCHAR(255) NOT NULL,
                version VARCHAR(255) NOT NULL,
                executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE KEY unique_migration (module_name, version)
            )
        "#;
        
        self.db.execute(sql).await
            .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))?;
        
        Ok(())
    }
    
    /// 执行迁移（非事务模式）
    ///
    /// # 说明
    /// 使用 yang-db::Database::execute 执行 SQL
    async fn run_migrations(&self, module: &dyn Module) -> Result<(), BaseError> {
        let module_name = module.name();
        
        for (version, sql) in module.migration_sql() {
            // 检查迁移是否已执行
            if self.is_migration_executed(module_name, &version).await? {
                log::debug!("迁移 {} v{} 已执行，跳过", module_name, version);
                continue;
            }
            
            log::info!("执行迁移: {} v{}", module_name, version);
            
            // 执行迁移 SQL（使用 yang-db::Database::execute）
            self.db.execute(&sql).await
                .map_err(|e| BaseError::MigrationFailed(
                    module_name.to_string(),
                    version.clone(),
                    e.to_string()
                ))?;
            
            // 记录迁移
            self.record_migration(module_name, &version).await?;
        }
        
        Ok(())
    }
    
    /// 执行迁移（事务模式）
    ///
    /// # 说明
    /// 使用 yang-db::Transaction::execute 执行 SQL
    async fn run_migrations_in_tx(
        &self,
        tx: &mut yang_db::Transaction,
        module: &dyn Module,
    ) -> Result<(), BaseError> {
        let module_name = module.name();
        
        for (version, sql) in module.migration_sql() {
            // 检查迁移是否已执行
            if self.is_migration_executed(module_name, &version).await? {
                log::debug!("迁移 {} v{} 已执行，跳过", module_name, version);
                continue;
            }
            
            log::info!("执行迁移: {} v{}", module_name, version);
            
            // 执行迁移 SQL（使用 yang-db::Transaction::execute）
            tx.execute(&sql).await
                .map_err(|e| BaseError::MigrationFailed(
                    module_name.to_string(),
                    version.clone(),
                    e.to_string()
                ))?;
            
            // 记录迁移
            let record_sql = format!(
                "INSERT INTO _migrations (module_name, version) VALUES ('{}', '{}')",
                module_name, version
            );
            tx.execute(&record_sql).await
                .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))?;
        }
        
        Ok(())
    }
    
    /// 检查迁移是否已执行
    ///
    /// # 说明
    /// 使用 yang-db::Database::pool 获取连接池，然后使用 sqlx 查询
    async fn is_migration_executed(
        &self,
        module_name: &str,
        version: &str,
    ) -> Result<bool, BaseError> {
        let sql = format!(
            "SELECT COUNT(*) as count FROM _migrations WHERE module_name = '{}' AND version = '{}'",
            module_name, version
        );
        
        let row: (i64,) = sqlx::query_as(&sql)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| BaseError::DatabaseQueryFailed(e.to_string()))?;
        
        Ok(row.0 > 0)
    }
    
    /// 记录迁移
    ///
    /// # 说明
    /// 使用 yang-db::Database::execute 执行 SQL
    async fn record_migration(
        &self,
        module_name: &str,
        version: &str,
    ) -> Result<(), BaseError> {
        let sql = format!(
            "INSERT INTO _migrations (module_name, version) VALUES ('{}', '{}')",
            module_name, version
        );
        
        self.db.execute(&sql).await
            .map_err(|e| BaseError::DatabaseExecuteFailed(e.to_string()))?;
        
        Ok(())
    }
}
```

### HttpClient 结构体

```rust
use reqwest::{Client, Method, Response as ReqwestResponse};
use std::sync::OnceLock;
use std::time::Duration;

/// 全局 HTTP 客户端实例
static GLOBAL_HTTP_CLIENT: OnceLock<HttpClient> = OnceLock::new();

/// HTTP 客户端
///
/// 提供 HTTP 请求构建和发送能力
pub struct HttpClient {
    /// reqwest 客户端
    client: Client,
    
    /// 默认超时时间（秒）
    default_timeout: Duration,
    
    /// 默认 Token（可选）
    default_token: Option<String>,
}

impl HttpClient {
    /// 创建新的 HTTP 客户端
    ///
    /// # 参数
    /// - timeout_secs: 默认超时时间（秒）
    ///
    /// # 返回
    /// - Ok(HttpClient): 客户端实例
    /// - Err(BaseError): 创建失败
    pub fn new(timeout_secs: u64) -> Result<Self, BaseError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| BaseError::HttpClientCreateFailed(e.to_string()))?;
        
        Ok(Self {
            client,
            default_timeout: Duration::from_secs(timeout_secs),
            default_token: None,
        })
    }
    
    /// 初始化全局 HTTP 客户端
    ///
    /// # 参数
    /// - timeout_secs: 默认超时时间（秒）
    ///
    /// # 返回
    /// - Ok(()): 初始化成功
    /// - Err(BaseError): 初始化失败
    pub fn init_global(timeout_secs: u64) -> Result<(), BaseError> {
        let client = Self::new(timeout_secs)?;
        
        GLOBAL_HTTP_CLIENT.set(client)
            .map_err(|_| BaseError::HttpClientAlreadyInitialized)?;
        
        log::info!("全局 HTTP 客户端已初始化");
        Ok(())
    }
    
    /// 获取全局 HTTP 客户端
    ///
    /// # 返回
    /// - Ok(&HttpClient): 客户端实例
    /// - Err(BaseError): 客户端未初始化
    pub fn global() -> Result<&'static HttpClient, BaseError> {
        GLOBAL_HTTP_CLIENT.get()
            .ok_or(BaseError::HttpClientNotInitialized)
    }
    
    /// 设置默认 Token
    ///
    /// # 参数
    /// - token: Token 字符串
    pub fn set_default_token(&mut self, token: String) {
        self.default_token = Some(token);
    }
    
    /// 创建请求构建器
    ///
    /// # 参数
    /// - method: HTTP 方法
    /// - url: 请求 URL
    ///
    /// # 返回
    /// - RequestBuilder: 请求构建器
    pub fn request(&self, method: Method, url: &str) -> RequestBuilder {
        RequestBuilder::new(
            self.client.clone(),
            method,
            url.to_string(),
            self.default_timeout,
            self.default_token.clone(),
        )
    }
    
    /// GET 请求
    pub fn get(&self, url: &str) -> RequestBuilder {
        self.request(Method::GET, url)
    }
    
    /// POST 请求
    pub fn post(&self, url: &str) -> RequestBuilder {
        self.request(Method::POST, url)
    }
    
    /// PUT 请求
    pub fn put(&self, url: &str) -> RequestBuilder {
        self.request(Method::PUT, url)
    }
    
    /// DELETE 请求
    pub fn delete(&self, url: &str) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }
    
    /// PATCH 请求
    pub fn patch(&self, url: &str) -> RequestBuilder {
        self.request(Method::PATCH, url)
    }
}
```


### RequestBuilder 结构体

```rust
use reqwest::{Client, Method, header::{HeaderMap, HeaderName, HeaderValue}};
use serde::Serialize;
use std::time::Duration;

/// HTTP 请求构建器
///
/// 提供链式调用接口构建 HTTP 请求
pub struct RequestBuilder {
    /// reqwest 客户端
    client: Client,
    
    /// HTTP 方法
    method: Method,
    
    /// 请求 URL
    url: String,
    
    /// 请求头
    headers: HeaderMap,
    
    /// 查询参数
    query_params: Vec<(String, String)>,
    
    /// 请求体
    body: Option<Vec<u8>>,
    
    /// 超时时间
    timeout: Duration,
    
    /// Token（可选）
    token: Option<String>,
}

impl RequestBuilder {
    /// 创建新的请求构建器
    pub(crate) fn new(
        client: Client,
        method: Method,
        url: String,
        timeout: Duration,
        token: Option<String>,
    ) -> Self {
        Self {
            client,
            method,
            url,
            headers: HeaderMap::new(),
            query_params: Vec::new(),
            body: None,
            timeout,
            token,
        }
    }
    
    /// 设置请求头
    ///
    /// # 参数
    /// - name: 请求头名称
    /// - value: 请求头值
    pub fn header(mut self, name: &str, value: &str) -> Self {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            self.headers.insert(name, value);
        }
        self
    }
    
    /// 批量设置请求头
    ///
    /// # 参数
    /// - headers: 请求头列表
    pub fn headers(mut self, headers: Vec<(&str, &str)>) -> Self {
        for (name, value) in headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                self.headers.insert(name, value);
            }
        }
        self
    }
    
    /// 设置 Content-Type
    pub fn content_type(self, content_type: &str) -> Self {
        self.header("Content-Type", content_type)
    }
    
    /// 设置 Bearer Token
    pub fn bearer_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }
    
    /// 设置 User-Agent
    pub fn user_agent(self, user_agent: &str) -> Self {
        self.header("User-Agent", user_agent)
    }
    
    /// 添加查询参数
    ///
    /// # 参数
    /// - key: 参数名
    /// - value: 参数值
    pub fn query(mut self, key: &str, value: &str) -> Self {
        self.query_params.push((key.to_string(), value.to_string()));
        self
    }
    
    /// 批量添加查询参数
    pub fn queries(mut self, params: Vec<(&str, &str)>) -> Self {
        for (key, value) in params {
            self.query_params.push((key.to_string(), value.to_string()));
        }
        self
    }
    
    /// 设置 JSON 请求体
    ///
    /// # 参数
    /// - json: 可序列化为 JSON 的数据
    pub fn json<T: Serialize>(mut self, json: &T) -> Result<Self, BaseError> {
        let json_str = serde_json::to_vec(json)
            .map_err(|e| BaseError::JsonSerializeFailed(e.to_string()))?;
        
        self.body = Some(json_str);
        self = self.content_type("application/json");
        
        Ok(self)
    }
    
    /// 设置表单请求体
    ///
    /// # 参数
    /// - form: 表单数据
    pub fn form(mut self, form: Vec<(&str, &str)>) -> Self {
        let form_str = form.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        
        self.body = Some(form_str.into_bytes());
        self = self.content_type("application/x-www-form-urlencoded");
        
        self
    }
    
    /// 设置原始字节请求体
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }
    
    /// 设置文本请求体
    pub fn text(mut self, text: &str) -> Self {
        self.body = Some(text.as_bytes().to_vec());
        self = self.content_type("text/plain");
        self
    }
    
    /// 设置超时时间
    ///
    /// # 参数
    /// - timeout_secs: 超时时间（秒）
    pub fn timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout = Duration::from_secs(timeout_secs);
        self
    }
    
    /// 发送请求
    ///
    /// # 返回
    /// - Ok(Response): 响应对象
    /// - Err(BaseError): 请求失败
    pub async fn send(self) -> Result<Response, BaseError> {
        // 构建请求
        let mut request = self.client
            .request(self.method, &self.url)
            .timeout(self.timeout);
        
        // 添加请求头
        request = request.headers(self.headers);
        
        // 添加 Token
        if let Some(token) = self.token {
            request = request.bearer_auth(token);
        }
        
        // 添加查询参数
        if !self.query_params.is_empty() {
            request = request.query(&self.query_params);
        }
        
        // 添加请求体
        if let Some(body) = self.body {
            request = request.body(body);
        }
        
        // 发送请求
        let response = request.send().await
            .map_err(|e| BaseError::HttpRequestFailed(e.to_string()))?;
        
        Ok(Response::new(response))
    }
}

/// HTTP 响应
///
/// 封装 reqwest::Response，提供便捷的响应处理方法
pub struct Response {
    /// reqwest 响应
    response: ReqwestResponse,
}

impl Response {
    /// 创建新的响应对象
    pub(crate) fn new(response: ReqwestResponse) -> Self {
        Self { response }
    }
    
    /// 获取状态码
    pub fn status(&self) -> u16 {
        self.response.status().as_u16()
    }
    
    /// 检查响应是否成功（2xx）
    pub fn is_success(&self) -> bool {
        self.response.status().is_success()
    }
    
    /// 获取响应头
    pub fn headers(&self) -> &HeaderMap {
        self.response.headers()
    }
    
    /// 获取响应体为文本
    pub async fn text(self) -> Result<String, BaseError> {
        self.response.text().await
            .map_err(|e| BaseError::HttpResponseParseFailed(e.to_string()))
    }
    
    /// 获取响应体为字节流
    pub async fn bytes(self) -> Result<Vec<u8>, BaseError> {
        self.response.bytes().await
            .map(|b| b.to_vec())
            .map_err(|e| BaseError::HttpResponseParseFailed(e.to_string()))
    }
    
    /// 获取响应体为 JSON
    pub async fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, BaseError> {
        self.response.json().await
            .map_err(|e| BaseError::JsonDeserializeFailed(e.to_string()))
    }
}
```

### TokenManager 结构体

```rust
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey, Algorithm};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Token 管理器
///
/// 提供 JWT Token 的生成、验证、解析和刷新功能
pub struct TokenManager {
    /// 编码密钥
    encoding_key: EncodingKey,
    
    /// 解码密钥
    decoding_key: DecodingKey,
    
    /// 算法
    algorithm: Algorithm,
    
    /// 签发者
    issuer: String,
    
    /// 受众
    audience: String,
    
    /// Access Token 有效期（秒）
    access_token_expiry: u64,
    
    /// Refresh Token 有效期（秒）
    refresh_token_expiry: u64,
}

/// Token 声明
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    /// 签发者
    pub iss: String,
    
    /// 主题（用户 ID）
    pub sub: String,
    
    /// 受众
    pub aud: String,
    
    /// 过期时间（Unix 时间戳）
    pub exp: u64,
    
    /// 生效时间（Unix 时间戳）
    pub nbf: u64,
    
    /// 签发时间（Unix 时间戳）
    pub iat: u64,
    
    /// JWT ID
    pub jti: String,
    
    /// Token 类型（access 或 refresh）
    pub token_type: String,
    
    /// 自定义声明
    #[serde(flatten)]
    pub custom: serde_json::Value,
}

impl TokenManager {
    /// 创建新的 Token 管理器（对称加密）
    ///
    /// # 参数
    /// - secret: 密钥
    /// - algorithm: 算法（HS256, HS384, HS512）
    /// - issuer: 签发者
    /// - audience: 受众
    /// - access_token_expiry: Access Token 有效期（秒）
    /// - refresh_token_expiry: Refresh Token 有效期（秒）
    pub fn new_symmetric(
        secret: &str,
        algorithm: Algorithm,
        issuer: String,
        audience: String,
        access_token_expiry: u64,
        refresh_token_expiry: u64,
    ) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            algorithm,
            issuer,
            audience,
            access_token_expiry,
            refresh_token_expiry,
        }
    }
    
    /// 创建新的 Token 管理器（非对称加密）
    ///
    /// # 参数
    /// - private_key: 私钥（PEM 格式）
    /// - public_key: 公钥（PEM 格式）
    /// - algorithm: 算法（RS256, RS384, RS512）
    /// - issuer: 签发者
    /// - audience: 受众
    /// - access_token_expiry: Access Token 有效期（秒）
    /// - refresh_token_expiry: Refresh Token 有效期（秒）
    pub fn new_asymmetric(
        private_key: &str,
        public_key: &str,
        algorithm: Algorithm,
        issuer: String,
        audience: String,
        access_token_expiry: u64,
        refresh_token_expiry: u64,
    ) -> Result<Self, BaseError> {
        let encoding_key = EncodingKey::from_rsa_pem(private_key.as_bytes())
            .map_err(|e| BaseError::TokenKeyInvalid(e.to_string()))?;
        
        let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
            .map_err(|e| BaseError::TokenKeyInvalid(e.to_string()))?;
        
        Ok(Self {
            encoding_key,
            decoding_key,
            algorithm,
            issuer,
            audience,
            access_token_expiry,
            refresh_token_expiry,
        })
    }
    
    /// 生成 Access Token
    ///
    /// # 参数
    /// - subject: 主题（用户 ID）
    /// - custom_claims: 自定义声明
    ///
    /// # 返回
    /// - Ok(String): Token 字符串
    /// - Err(BaseError): 生成失败
    pub fn generate_access_token(
        &self,
        subject: &str,
        custom_claims: serde_json::Value,
    ) -> Result<String, BaseError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let claims = TokenClaims {
            iss: self.issuer.clone(),
            sub: subject.to_string(),
            aud: self.audience.clone(),
            exp: now + self.access_token_expiry,
            nbf: now,
            iat: now,
            jti: uuid::Uuid::new_v4().to_string(),
            token_type: "access".to_string(),
            custom: custom_claims,
        };
        
        let mut header = Header::new(self.algorithm);
        header.typ = Some("JWT".to_string());
        
        encode(&header, &claims, &self.encoding_key)
            .map_err(|e| BaseError::TokenGenerateFailed(e.to_string()))
    }
    
    /// 生成 Refresh Token
    ///
    /// # 参数
    /// - subject: 主题（用户 ID）
    ///
    /// # 返回
    /// - Ok(String): Token 字符串
    /// - Err(BaseError): 生成失败
    pub fn generate_refresh_token(&self, subject: &str) -> Result<String, BaseError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let claims = TokenClaims {
            iss: self.issuer.clone(),
            sub: subject.to_string(),
            aud: self.audience.clone(),
            exp: now + self.refresh_token_expiry,
            nbf: now,
            iat: now,
            jti: uuid::Uuid::new_v4().to_string(),
            token_type: "refresh".to_string(),
            custom: serde_json::Value::Null,
        };
        
        let mut header = Header::new(self.algorithm);
        header.typ = Some("JWT".to_string());
        
        encode(&header, &claims, &self.encoding_key)
            .map_err(|e| BaseError::TokenGenerateFailed(e.to_string()))
    }
    
    /// 生成 Token 对（Access Token + Refresh Token）
    ///
    /// # 参数
    /// - subject: 主题（用户 ID）
    /// - custom_claims: 自定义声明
    ///
    /// # 返回
    /// - Ok((access_token, refresh_token)): Token 对
    /// - Err(BaseError): 生成失败
    pub fn generate_token_pair(
        &self,
        subject: &str,
        custom_claims: serde_json::Value,
    ) -> Result<(String, String), BaseError> {
        let access_token = self.generate_access_token(subject, custom_claims)?;
        let refresh_token = self.generate_refresh_token(subject)?;
        
        Ok((access_token, refresh_token))
    }
    
    /// 验证 Token
    ///
    /// # 参数
    /// - token: Token 字符串
    ///
    /// # 返回
    /// - Ok(TokenClaims): Token 声明
    /// - Err(BaseError): 验证失败
    pub fn verify_token(&self, token: &str) -> Result<TokenClaims, BaseError> {
        let mut validation = Validation::new(self.algorithm);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        
        let token_data = decode::<TokenClaims>(token, &self.decoding_key, &validation)
            .map_err(|e| BaseError::TokenVerifyFailed(e.to_string()))?;
        
        Ok(token_data.claims)
    }
    
    /// 解析 Token（不验证签名）
    ///
    /// # 参数
    /// - token: Token 字符串
    ///
    /// # 返回
    /// - Ok(TokenClaims): Token 声明
    /// - Err(BaseError): 解析失败
    pub fn parse_token_unsafe(&self, token: &str) -> Result<TokenClaims, BaseError> {
        let mut validation = Validation::new(self.algorithm);
        validation.insecure_disable_signature_validation();
        validation.validate_exp = false;
        validation.validate_nbf = false;
        
        let token_data = decode::<TokenClaims>(token, &self.decoding_key, &validation)
            .map_err(|e| BaseError::TokenParseFailed(e.to_string()))?;
        
        Ok(token_data.claims)
    }
    
    /// 检查 Token 是否即将过期
    ///
    /// # 参数
    /// - token: Token 字符串
    /// - threshold_secs: 阈值（秒），如果剩余时间少于此值则返回 true
    ///
    /// # 返回
    /// - Ok(bool): 是否即将过期
    /// - Err(BaseError): 检查失败
    pub fn is_token_expiring_soon(
        &self,
        token: &str,
        threshold_secs: u64,
    ) -> Result<bool, BaseError> {
        let claims = self.verify_token(token)?;
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let remaining = claims.exp.saturating_sub(now);
        
        Ok(remaining < threshold_secs)
    }
    
    /// 使用 Refresh Token 刷新 Access Token
    ///
    /// # 参数
    /// - refresh_token: Refresh Token 字符串
    /// - custom_claims: 新的自定义声明
    ///
    /// # 返回
    /// - Ok(String): 新的 Access Token
    /// - Err(BaseError): 刷新失败
    pub fn refresh_access_token(
        &self,
        refresh_token: &str,
        custom_claims: serde_json::Value,
    ) -> Result<String, BaseError> {
        // 验证 Refresh Token
        let claims = self.verify_token(refresh_token)?;
        
        // 检查 Token 类型
        if claims.token_type != "refresh" {
            return Err(BaseError::TokenTypeInvalid("期望 refresh token".to_string()));
        }
        
        // 生成新的 Access Token
        self.generate_access_token(&claims.sub, custom_claims)
    }
}
```

## 数据模型

### 核心数据结构

#### ModuleMetadata

```rust
/// 模块元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleMetadata {
    /// 模块名称
    pub name: String,
    
    /// 模块版本
    pub version: String,
    
    /// 模块描述
    pub description: Option<String>,
    
    /// 模块作者
    pub author: Option<String>,
    
    /// 依赖列表
    pub dependencies: Vec<String>,
    
    /// 是否已初始化
    pub initialized: bool,
}
```

#### MigrationRecord

```rust
/// 迁移记录
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MigrationRecord {
    /// 记录 ID
    pub id: i32,
    
    /// 模块名称
    pub module_name: String,
    
    /// 迁移版本
    pub version: String,
    
    /// 执行时间
    pub executed_at: chrono::NaiveDateTime,
}
```

#### HttpConfig

```rust
/// HTTP 客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// 默认超时时间（秒）
    pub timeout_secs: u64,
    
    /// 默认 User-Agent
    pub user_agent: Option<String>,
    
    /// 是否启用日志
    pub enable_logging: bool,
    
    /// 最大重试次数
    pub max_retries: u32,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            user_agent: Some("yang-base/0.1.0".to_string()),
            enable_logging: false,
            max_retries: 3,
        }
    }
}
```

#### TokenConfig

```rust
/// Token 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenConfig {
    /// 密钥或私钥
    pub secret_or_private_key: String,
    
    /// 公钥（非对称加密时使用）
    pub public_key: Option<String>,
    
    /// 算法
    pub algorithm: String,
    
    /// 签发者
    pub issuer: String,
    
    /// 受众
    pub audience: String,
    
    /// Access Token 有效期（秒）
    pub access_token_expiry: u64,
    
    /// Refresh Token 有效期（秒）
    pub refresh_token_expiry: u64,
}

impl Default for TokenConfig {
    fn default() -> Self {
        Self {
            secret_or_private_key: String::new(),
            public_key: None,
            algorithm: "HS256".to_string(),
            issuer: "yang-base".to_string(),
            audience: "yang-app".to_string(),
            access_token_expiry: 3600,      // 1 小时
            refresh_token_expiry: 2592000,  // 30 天
        }
    }
}
```

### 数据库表结构

#### 迁移记录表

```sql
CREATE TABLE IF NOT EXISTS _migrations (
    id INT AUTO_INCREMENT PRIMARY KEY,
    module_name VARCHAR(255) NOT NULL COMMENT '模块名称',
    version VARCHAR(255) NOT NULL COMMENT '迁移版本',
    executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '执行时间',
    UNIQUE KEY unique_migration (module_name, version),
    INDEX idx_module_name (module_name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='数据库迁移记录表';
```


## 错误处理

### 错误类型设计

系统使用统一的错误类型 BaseError，封装所有模块的错误情况。

```rust
use thiserror::Error;

/// yang-base 统一错误类型
#[derive(Debug, Error)]
pub enum BaseError {
    // ========== 模块管理错误 ==========
    
    #[error("模块已注册: {0}")]
    ModuleAlreadyRegistered(String),
    
    #[error("模块未找到: {0}")]
    ModuleNotFound(String),
    
    #[error("模块注册失败: {0}, 原因: {1}")]
    ModuleRegisterFailed(String, String),
    
    #[error("模块初始化失败: {0}, 原因: {1}")]
    ModuleInitFailed(String, String),
    
    #[error("模块关闭失败: {0}, 原因: {1}")]
    ModuleShutdownFailed(String, String),
    
    #[error("模块依赖缺失: {0} 依赖 {1}")]
    ModuleDependencyMissing(String, String),
    
    #[error("模块循环依赖: {0}")]
    ModuleCircularDependency(String),
    
    #[error("模块配置无效: {0}")]
    ModuleConfigInvalid(String),
    
    // ========== 数据库错误 ==========
    
    #[error("数据库连接失败: {0}")]
    DatabaseConnectionFailed(String),
    
    #[error("数据库已初始化")]
    DatabaseAlreadyInitialized,
    
    #[error("数据库未初始化")]
    DatabaseNotInitialized,
    
    #[error("数据库查询失败: {0}")]
    DatabaseQueryFailed(String),
    
    #[error("数据库执行失败: {0}")]
    DatabaseExecuteFailed(String),
    
    #[error("数据库事务失败: {0}")]
    DatabaseTransactionFailed(String),
    
    #[error("数据库迁移失败: 模块 {0}, 版本 {1}, 原因: {2}")]
    MigrationFailed(String, String, String),
    
    // ========== HTTP 客户端错误 ==========
    
    #[error("HTTP 客户端创建失败: {0}")]
    HttpClientCreateFailed(String),
    
    #[error("HTTP 客户端已初始化")]
    HttpClientAlreadyInitialized,
    
    #[error("HTTP 客户端未初始化")]
    HttpClientNotInitialized,
    
    #[error("HTTP 请求失败: {0}")]
    HttpRequestFailed(String),
    
    #[error("HTTP 响应解析失败: {0}")]
    HttpResponseParseFailed(String),
    
    // ========== Token 管理错误 ==========
    
    #[error("Token 密钥无效: {0}")]
    TokenKeyInvalid(String),
    
    #[error("Token 生成失败: {0}")]
    TokenGenerateFailed(String),
    
    #[error("Token 验证失败: {0}")]
    TokenVerifyFailed(String),
    
    #[error("Token 解析失败: {0}")]
    TokenParseFailed(String),
    
    #[error("Token 类型无效: {0}")]
    TokenTypeInvalid(String),
    
    // ========== 序列化错误 ==========
    
    #[error("JSON 序列化失败: {0}")]
    JsonSerializeFailed(String),
    
    #[error("JSON 反序列化失败: {0}")]
    JsonDeserializeFailed(String),
    
    // ========== 通用错误 ==========
    
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    #[error("IO 错误: {0}")]
    IoError(String),
    
    #[error("未知错误: {0}")]
    Unknown(String),
}

// 从 yang_db::DbError 转换
impl From<yang_db::DbError> for BaseError {
    fn from(err: yang_db::DbError) -> Self {
        BaseError::DatabaseQueryFailed(err.to_string())
    }
}

// 从 std::io::Error 转换
impl From<std::io::Error> for BaseError {
    fn from(err: std::io::Error) -> Self {
        BaseError::IoError(err.to_string())
    }
}
```

### 错误处理策略

#### 1. 错误传播

- 使用 Result<T, BaseError> 作为返回类型
- 使用 ? 操作符简化错误传播
- 在模块边界进行错误转换

```rust
// 示例：错误传播
pub async fn register_module<M: Module + 'static>(
    &self,
    module: M,
) -> Result<(), BaseError> {
    // 调用可能失败的操作
    module.on_register().await
        .map_err(|e| BaseError::ModuleRegisterFailed(
            module.name().to_string(),
            e.to_string()
        ))?;
    
    // 继续其他操作...
    Ok(())
}
```

#### 2. 错误日志

- 在错误发生时记录详细的上下文信息
- 使用不同的日志级别：
  - error!: 严重错误，需要立即处理
  - warn!: 警告信息，可能影响功能
  - info!: 一般信息，记录关键操作
  - debug!: 调试信息，详细的执行流程

```rust
// 示例：错误日志
if let Err(e) = module.on_init().await {
    log::error!("模块 {} 初始化失败: {}", name, e);
    return Err(BaseError::ModuleInitFailed(name.to_string(), e.to_string()));
}
```

#### 3. 错误恢复

- 对于可恢复的错误，提供重试机制
- 对于不可恢复的错误，快速失败并清理资源

```rust
// 示例：HTTP 请求重试
pub async fn send_with_retry(&self, max_retries: u32) -> Result<Response, BaseError> {
    let mut attempts = 0;
    
    loop {
        match self.send().await {
            Ok(response) => return Ok(response),
            Err(e) => {
                attempts += 1;
                if attempts >= max_retries {
                    return Err(e);
                }
                log::warn!("请求失败，重试 {}/{}: {}", attempts, max_retries, e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
```

## 测试策略

### 测试方法分类

根据功能特性，系统采用不同的测试方法：

#### 1. 单元测试（Unit Tests）

**适用范围**：
- 模块注册和查找逻辑
- 依赖关系验证
- 配置验证
- 错误处理逻辑

**测试重点**：
- 正常情况下的功能正确性
- 边界条件处理
- 错误情况处理

**示例**：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_register_module() {
        let manager = ModuleManager::new();
        let module = TestModule::new("test-module");
        
        // 测试正常注册
        assert!(manager.register(module).await.is_ok());
        
        // 测试重复注册
        let module2 = TestModule::new("test-module");
        assert!(manager.register(module2).await.is_err());
    }
    
    #[tokio::test]
    async fn test_module_not_found() {
        let manager = ModuleManager::new();
        
        // 测试查找不存在的模块
        assert!(manager.get("non-existent").await.is_none());
    }
}
```

#### 2. 集成测试（Integration Tests）

**适用范围**：
- 数据库初始化流程（使用 yang-db 库）
- 模块生命周期管理
- HTTP 客户端与外部服务交互
- Token 管理器与 HTTP 客户端集成

**测试重点**：
- 模块间的协作
- 端到端的功能流程
- 真实环境下的行为
- yang-db 库的集成

**示例**：

```rust
#[tokio::test]
async fn test_database_initialization() {
    // 创建测试数据库（使用 yang-db::Database）
    let db_url = "mysql://root:password@localhost/test_db";
    let db = Database::connect(db_url).await.unwrap();
    
    // 创建模块管理器
    let manager = ModuleManager::new();
    manager.register(TestModule::new("module1")).await.unwrap();
    manager.register(TestModule::new("module2")).await.unwrap();
    
    // 初始化数据库（使用 yang-db 方法）
    let initializer = DatabaseInitializer::new(db, false);
    assert!(initializer.initialize_all(&manager).await.is_ok());
    
    // 验证表已创建
    // ...
}
```

#### 3. 属性测试（Property-Based Tests）

**适用范围**：
- JWT Token 的 round-trip 属性
- 模块依赖关系的拓扑排序
- 配置验证的一致性

**测试重点**：
- 通用属性在大量随机输入下的正确性
- 边界情况的自动发现

**为什么不对所有功能使用 PBT**：

根据 PBT 适用性评估：

1. **不适合 PBT 的部分**：
   - **数据库初始化**：这是基础设施操作，涉及外部 MySQL 服务和 yang-db 库，行为不随输入变化，运行 100 次与运行 1 次没有区别
   - **HTTP 客户端**：涉及外部 API 调用，成本高，行为由外部服务决定，不适合大量随机测试
   - **模块注册**：主要是状态管理，用示例测试更合适

2. **适合 PBT 的部分**：
   - **JWT Token**：有明确的 round-trip 属性（生成→验证→解析应该保持一致）
   - **依赖排序**：拓扑排序算法有明确的不变量

**JWT Token 属性测试示例**：

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_token_round_trip(
        user_id in "[a-z0-9]{8,32}",
        role in prop::option::of("[a-z]{4,10}"),
    ) {
        // 创建 Token 管理器
        let manager = TokenManager::new_symmetric(
            "test-secret",
            Algorithm::HS256,
            "test-issuer".to_string(),
            "test-audience".to_string(),
            3600,
            86400,
        );
        
        // 创建自定义声明
        let custom = serde_json::json!({
            "role": role,
        });
        
        // 生成 Token
        let token = manager.generate_access_token(&user_id, custom.clone()).unwrap();
        
        // 验证 Token
        let claims = manager.verify_token(&token).unwrap();
        
        // 属性：subject 应该保持一致
        prop_assert_eq!(claims.sub, user_id);
        
        // 属性：自定义声明应该保持一致
        prop_assert_eq!(claims.custom.get("role"), custom.get("role"));
        
        // 属性：Token 类型应该是 access
        prop_assert_eq!(claims.token_type, "access");
    }
}
```

#### 4. 性能测试（Performance Tests）

**适用范围**：
- 模块管理器的并发访问性能
- 数据库连接池的性能（yang-db 库）
- HTTP 客户端的并发请求性能

**测试重点**：
- 响应时间
- 吞吐量
- 资源使用

#### 5. 安全测试（Security Tests）

**适用范围**：
- Token 验证的安全性
- SQL 注入防护（yang-db 库提供）
- 敏感信息泄露

**测试重点**：
- 认证和授权
- 输入验证
- 错误信息不泄露敏感数据

### 测试覆盖率目标

- **单元测试覆盖率**：≥ 80%
- **集成测试覆盖率**：核心流程 100%
- **属性测试**：每个属性至少 100 次迭代

### 测试组织

```
crates/yang-base/
├── src/
│   ├── module/
│   │   ├── mod.rs
│   │   ├── manager.rs
│   │   └── __tests__/
│   │       ├── manager_test.rs
│   │       └── module_test.rs
│   ├── database/
│   │   ├── mod.rs
│   │   ├── global.rs
│   │   ├── initializer.rs
│   │   └── __tests__/
│   │       ├── global_test.rs
│   │       └── initializer_test.rs
│   ├── http/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── request.rs
│   │   └── __tests__/
│   │       ├── client_test.rs
│   │       └── request_test.rs
│   └── token/
│       ├── mod.rs
│       ├── manager.rs
│       └── __tests__/
│           ├── manager_test.rs
│           └── properties_test.rs  # 属性测试
└── tests/
    ├── integration_test.rs
    └── performance_test.rs
```

### 测试依赖

```toml
[dev-dependencies]
tokio = { version = "1.0", features = ["full", "test-util"] }
proptest = "1.0"  # 属性测试
criterion = "0.5"  # 性能测试
mockito = "1.0"   # HTTP Mock
testcontainers = "0.15"  # 数据库容器测试
yang-db = { path = "../yang-db", features = ["test-utils"] }  # yang-db 测试工具
```

### 持续集成

- 每次提交自动运行所有测试
- 代码覆盖率报告
- 性能回归检测
- 安全漏洞扫描

