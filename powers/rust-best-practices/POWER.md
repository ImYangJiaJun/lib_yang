---
name: "rust-best-practices"
displayName: "Rust 最佳实践"
description: "Rust 开发的最佳实践指南，包括代码规范、错误处理、性能优化和安全编码模式"
keywords: ["rust", "best-practices", "coding-standards", "error-handling", "performance"]
author: "YANG Team"
---

# Rust 最佳实践

## 概述

本 Power 提供了 Rust 开发的全面最佳实践指南，涵盖代码规范、错误处理、性能优化和安全编码模式。这些实践基于 Rust 社区的共识和实际项目经验，帮助开发者编写更清晰、更健壮、更高效的 Rust 代码。

## 核心原则

### 1. 所有权和借用优先
- 优先使用借用而非所有权转移
- 使用 `&` 和 `&mut` 明确表达意图
- 避免不必要的 `clone()`

### 2. 错误处理明确化
- 使用 `Result<T, E>` 处理可恢复错误
- 使用 `Option<T>` 处理可能缺失的值
- 避免 `unwrap()` 和 `panic!()`，除非有充分理由

### 3. 类型安全至上
- 利用类型系统防止错误
- 使用 newtype 模式增强类型安全
- 优先使用强类型而非原始类型

## 常见模式

### 模式 1：错误处理

**问题：** 如何优雅地处理错误？

**推荐做法：**
```rust
// 好的做法 - 使用 Result 和 ? 操作符
fn read_config(path: &str) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config = serde_json::from_str(&content)?;
    Ok(config)
}

// 不好的做法 - 使用 unwrap
fn read_config_bad(path: &str) -> Config {
    let content = std::fs::read_to_string(path).unwrap(); // 可能 panic
    serde_json::from_str(&content).unwrap() // 可能 panic
}
```

**原因：**
- `Result` 强制调用者处理错误
- `?` 操作符简化错误传播
- 避免程序意外崩溃

### 模式 2：借用检查器友好的代码

**问题：** 如何避免与借用检查器斗争？

**推荐做法：**
```rust
// 好的做法 - 缩小借用范围
fn process_data(data: &mut Vec<String>) {
    {
        let first = &data[0]; // 借用在这个作用域内
        println!("First: {}", first);
    } // 借用结束
    
    data.push("new item".to_string()); // 可以修改
}

// 不好的做法 - 借用时间过长
fn process_data_bad(data: &mut Vec<String>) {
    let first = &data[0]; // 借用持续到函数结束
    data.push("new item".to_string()); // 编译错误！
    println!("First: {}", first);
}
```

### 模式 3：使用 Builder 模式

**问题：** 如何处理有多个可选参数的结构体？

**推荐做法：**
```rust
// 好的做法 - Builder 模式
pub struct Config {
    host: String,
    port: u16,
    timeout: Option<u64>,
    retry: Option<u32>,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct ConfigBuilder {
    host: Option<String>,
    port: Option<u16>,
    timeout: Option<u64>,
    retry: Option<u32>,
}

impl ConfigBuilder {
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }
    
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }
    
    pub fn build(self) -> Result<Config, String> {
        Ok(Config {
            host: self.host.ok_or("host is required")?,
            port: self.port.unwrap_or(8080),
            timeout: self.timeout,
            retry: self.retry,
        })
    }
}

// 使用示例
let config = Config::builder()
    .host("localhost")
    .port(3000)
    .build()?;
```

## 命名规范

### 变量和函数
- 使用蛇形命名法（snake_case）
- 名称应该描述性强且有意义
- 布尔值使用 `is_`、`has_`、`can_` 前缀

```rust
// 好的命名
let user_count = 10;
let is_valid = true;
fn calculate_total_price() -> f64 { }

// 不好的命名
let uc = 10;
let valid = true;
fn calc() -> f64 { }
```

### 类型和 Trait
- 使用大驼峰命名法（PascalCase）
- 类型名应该是名词
- Trait 名应该是形容词或动词

```rust
// 好的命名
struct UserAccount { }
trait Serializable { }
enum PaymentStatus { }

// 不好的命名
struct user_account { }
trait serialize { }
```

## 性能优化

### 1. 避免不必要的分配
```rust
// 好 - 使用借用
fn process(s: &str) {
    println!("{}", s);
}

// 不好 - 不必要的所有权转移
fn process_bad(s: String) {
    println!("{}", s);
}
```

### 2. 使用迭代器而非循环
```rust
// 好 - 使用迭代器（零成本抽象）
let sum: i32 = numbers.iter().filter(|&&x| x > 0).sum();

// 可以，但不够优雅
let mut sum = 0;
for &num in &numbers {
    if num > 0 {
        sum += num;
    }
}
```

### 3. 预分配容量
```rust
// 好 - 预分配
let mut vec = Vec::with_capacity(1000);
for i in 0..1000 {
    vec.push(i);
}

// 不好 - 多次重新分配
let mut vec = Vec::new();
for i in 0..1000 {
    vec.push(i); // 可能触发多次重新分配
}
```

## 测试最佳实践

### 1. 测试组织
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addition() {
        assert_eq!(add(2, 2), 4);
    }

    #[test]
    fn test_edge_case() {
        assert_eq!(add(0, 0), 0);
    }
}
```

### 2. 使用 Result 进行测试
```rust
#[test]
fn test_with_result() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config("test.json")?;
    assert_eq!(config.port, 8080);
    Ok(())
}
```

## 文档注释

### 公开 API 必须有文档
```rust
/// 计算两个数的和
///
/// # 参数
///
/// * `a` - 第一个加数
/// * `b` - 第二个加数
///
/// # 返回值
///
/// 返回 `a` 和 `b` 的和
///
/// # 示例
///
/// ```
/// let result = add(2, 3);
/// assert_eq!(result, 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

## 常见陷阱

### 1. 过度使用 clone()
```rust
// 不好 - 不必要的克隆
fn process(data: Vec<String>) {
    let copy = data.clone(); // 昂贵的操作
    // 使用 copy...
}

// 好 - 使用借用
fn process(data: &[String]) {
    // 直接使用 data...
}
```

### 2. 忽略编译器警告
- 编译器警告通常指出潜在问题
- 使用 `#[allow(dead_code)]` 等属性时要谨慎
- 定期运行 `cargo clippy` 检查代码质量

### 3. 不处理 Result
```rust
// 不好 - 忽略错误
let _ = risky_operation();

// 好 - 明确处理
match risky_operation() {
    Ok(value) => println!("Success: {}", value),
    Err(e) => eprintln!("Error: {}", e),
}
```

## 工具推荐

- **rustfmt** - 自动格式化代码
- **clippy** - Lint 工具，检查常见错误和改进建议
- **cargo-audit** - 检查依赖的安全漏洞
- **cargo-outdated** - 检查过时的依赖

## 参考资源

- [Rust 官方文档](https://doc.rust-lang.org/)
- [Rust API 指南](https://rust-lang.github.io/api-guidelines/)
- [Rust 设计模式](https://rust-unofficial.github.io/patterns/)
- [Effective Rust](https://www.lurklurk.org/effective-rust/)

---

**类型：** Knowledge Base Power（知识库型）
**无需 MCP 配置** - 这是纯文档指南
