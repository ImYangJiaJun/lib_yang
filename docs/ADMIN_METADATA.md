# 可选后台元数据

`yang-base` 的 `admin-metadata` feature 提供纯展示描述。它默认关闭、不引入新依赖，也不持有或修改 Action dispatch、TableQuery 或 ApiCatalog。

## 描述与引用

- `AdminDisplayKind` 支持 menu、button、list、tree、form；`AdminMetadata` 另有 icon、group 与 order 属性。
- `AdminTarget::action(module, action)` 引用 ModuleRouter 的稳定名称。
- `AdminTarget::table(module, table)` 引用模块内 TableConfig 的稳定名称。
- `AdminTarget::api_operation(operation_id)` 引用 ApiCatalog 的 operation id。
- 所有稳定 ID 使用严格 ASCII 语法；`AdminMetadataRegistry` 拒绝重复 ID 并按 order/id 确定性排序。

这些引用不会自动注册路由、创建表、改变鉴权或覆盖核心 descriptor。应用适配层可以在启动时把引用与自己的 ApiCatalog/TableConfig 清单对账。

## 明确不做

审核流、状态机、审批权限和业务表结构属于业务插件能力，不作为所有 Action 的基础字段。若未来需要后台 UI 协议、序列化格式或前端组件依赖，应在独立 crate/RFC 中设计，不能把展示层依赖带入关闭 feature 的核心构建。
