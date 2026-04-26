# 依赖库版本更新总结

**更新日期**: 2026-04-26  
**更新人员**: 用户

---

## 📦 更新的依赖

### yang-base

| 依赖库 | 旧版本 | 新版本 | 变更类型 |
|--------|--------|--------|---------|
| reqwest | 0.12 | 0.13.2 | 次要版本 |
| jsonwebtoken | 9.0 | 10.3.0 | 主要版本 |
| uuid | 1.0 | 1.23.1 | 补丁版本 |

---

## 🔧 必要的代码修改

### 1. reqwest 0.13.2 更新

**问题**: `query` 方法需要 `query` feature

**解决方案**:
```toml
# 修改前
reqwest = { version = "0.13.2", features = ["json"] }

# 修改后
reqwest = { version = "0.13.2", features = ["json", "query"] }
```

**影响文件**: `crates/yang-base/Cargo.toml`

---

### 2. jsonwebtoken 10.3.0 更新

#### 问题 1: 需要显式选择加密提供者

**错误信息**:
```
Could not automatically determine the process-level CryptoProvider from jsonwebtoken crate features.
Call CryptoProvider::install_default() before this point to select a provider manually, or make 
sure exactly one of the 'rust_crypto' and 'aws_lc_rs' features is enabled.
```

**解决方案**:
```toml
# 修改前
jsonwebtoken = "10.3.0"

# 修改后
jsonwebtoken = { version = "10.3.0", features = ["aws_lc_rs"] }
```

**影响文件**: `crates/yang-base/Cargo.toml`

#### 问题 2: `insecure_disable_signature_validation()` 方法已弃用

**错误信息**:
```
warning: use of deprecated method `jsonwebtoken::Validation::insecure_disable_signature_validation`: 
Use `jsonwebtoken::dangerous::insecure_decode` if you require this functionality.
```

**解决方案**:
```rust
// 修改前
pub fn parse_token_unsafe(&self, token: &str) -> Result<TokenClaims, BaseError> {
    let mut validation = Validation::new(self.algorithm);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;
    validation.set_required_spec_claims::<&str>(&[]);

    let token_data = decode::<TokenClaims>(token, &self.decoding_key, &validation)
        .map_err(|e| BaseError::TokenParseFailed(e.to_string()))?;

    Ok(token_data.claims)
}

// 修改后
pub fn parse_token_unsafe(&self, token: &str) -> Result<TokenClaims, BaseError> {
    // 使用 dangerous::insecure_decode 进行不安全解析
    // 注意：此方法不验证签名、过期时间等,仅用于调试
    let token_data = jsonwebtoken::dangerous::insecure_decode::<TokenClaims>(token)
        .map_err(|e| BaseError::TokenParseFailed(e.to_string()))?;

    Ok(token_data.claims)
}
```

**影响文件**: `crates/yang-base/src/token/manager.rs`

---

## ✅ 验证结果

### 编译检查
```bash
cargo check
```
**结果**: ✅ 通过

### 单元测试
```bash
cargo test --lib
```
**结果**: ✅ 全部通过
- yang-base: 286 个测试通过
- yang-db: 184 个测试通过
- yang-pcg: 1 个测试通过

**总计**: 471 个测试全部通过

---

## 📝 变更说明

### reqwest 0.13.2
- 新增 `query` feature 要求
- API 保持向后兼容
- 性能和安全性改进

### jsonwebtoken 10.3.0
- **重大变更**: 需要显式选择加密提供者 (`aws_lc_rs` 或 `rust_crypto`)
- **API 变更**: `insecure_disable_signature_validation()` 已弃用,改用 `dangerous::insecure_decode()`
- 修复了时间验证相关的安全漏洞 (CVE-2026-25537)
- 改进了类型安全性

### uuid 1.23.1
- 补丁版本更新
- 无 API 变更
- 性能优化和 bug 修复

---

## 🔒 安全性改进

### jsonwebtoken 10.3.0 安全修复
- 修复了类型混淆漏洞
- 修复了时间验证绕过问题
- 加强了 "Not Before" 检查

**建议**: 强烈建议升级到 10.3.0 以获得安全修复。

---

## 📚 参考资料

- [reqwest 0.13 CHANGELOG](https://github.com/seanmonstar/reqwest/blob/master/CHANGELOG.md)
- [jsonwebtoken 10.3 CHANGELOG](https://github.com/Keats/jsonwebtoken/blob/master/CHANGELOG.md)
- [jsonwebtoken CVE-2026-25537](https://ubuntu.com/security/CVE-2026-25537)

---

## ⚠️ 注意事项

1. **加密提供者选择**: 我们选择了 `aws_lc_rs` 作为加密提供者,这是 AWS 提供的高性能加密库
2. **不安全解析**: `parse_token_unsafe` 方法仅用于调试,不应在生产环境中用于身份验证
3. **向后兼容**: 除了 jsonwebtoken 的 API 变更外,其他依赖保持向后兼容

---

**更新状态**: ✅ 已完成  
**测试状态**: ✅ 全部通过  
**提交状态**: 待提交
