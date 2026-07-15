# TableConfig 与数据库 Schema

`TableConfig` 是运行期访问、字段校验、权限、筛选/排序和关联描述的契约，不自动声明自己是数据库 DDL、迁移、索引、触发器或默认值的唯一真相。

## 可选兼容验证

- `TableConfig::validate_schema(&[SchemaColumn])` 对任意已获取的列快照做纯内存验证。
- 启用 `mysql` 时，`DatabaseInitializer::validate_table_config` 从 `information_schema.columns` 只读列快照后调用同一验证器。
- 报告包括声明字段缺失、存储类型不足和必填字段仍允许 NULL。字符串列必须能容纳声明的 `max_length`。
- 数据库额外列被忽略，因为迁移、触发器或其他消费者可以合法拥有这些列。
- `ForeignKey` 只验证本地列存在与 NULL 约束；TableConfig 没有目标键的物理类型信息，不能据此证明外键 DDL 完整。

## 明确不做

本阶段不比较或生成索引、外键约束、默认值、字符集、排序规则、分区、触发器和存储引擎，也不生成自动 ALTER 或回滚 SQL。这些能力需要独立 RFC、锁表/在线 DDL 策略和灾难恢复设计。
