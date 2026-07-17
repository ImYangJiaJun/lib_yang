//! 重构前后共同运行的 YANG 核心热路径基准。
//!
//! 本文件刻意只测无网络路径。数据库执行、事务与关系批量加载由 Docker 基准另行覆盖。

use async_trait::async_trait;
use criterion::{criterion_group, criterion_main, Criterion};
use jsonwebtoken::Algorithm;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;
use sqlx::mysql::MySqlPoolOptions;
use std::hint::black_box;
use std::sync::Arc;
use yang_base::action::{Action as ActionHandler, ActionContext, DynAction, Request};
use yang_base::definition::{
    ActionName, ActionRef, ActionSpec, AddonName, AddonSpec, AppBuilder, HttpMethod, ModuleName,
    ModuleSpec, ParamInput, RouteSpec, Str,
};
use yang_base::table::{Field, Table};
use yang_base::token::TokenManager;
use yang_base::tools::{Tools, ToolsBuilder};
use yang_base::{Action, BaseError};
use yang_db::{
    CompareOp, Database, DatabaseConfig, FieldRef as DbFieldRef, SortOrder as DbSortOrder,
    TableRef as DbTableRef,
};

yang_base::params! {
    #[derive(Serialize)]
    EchoInput {
        message: Str::new().require(true).max_length(64),
    }
}

#[derive(Debug, Serialize, JsonSchema)]
struct EchoOutput {
    message: String,
}

#[derive(Action)]
#[action(name = "echo", display_name = "基准回显", public)]
struct EchoAction;

#[async_trait]
impl ActionHandler for EchoAction {
    type Input = EchoInput;
    type Output = EchoOutput;

    async fn index(
        &self,
        _ctx: ActionContext,
        input: Self::Input,
    ) -> Result<Self::Output, BaseError> {
        Ok(EchoOutput {
            message: input.message,
        })
    }
}

fn tools() -> Arc<Tools> {
    let manager = TokenManager::new_symmetric(
        "runtime_baseline_secret",
        Algorithm::HS256,
        "benchmark".to_string(),
        "benchmark".to_string(),
        3_600,
        86_400,
    );
    Arc::new(
        ToolsBuilder::new()
            .token(manager)
            .extension(7_u64)
            .build()
            .expect("基准 Tools 应构建成功"),
    )
}

fn context(tools: &Arc<Tools>) -> ActionContext {
    ActionContext::new(
        Request::new(json!({ "message": "hello" })),
        Arc::clone(tools),
    )
}

fn app_builder() -> AppBuilder {
    let module = ModuleName::new("bench.echo").expect("固定 Module 名称有效");
    let action = ActionName::new("echo").expect("固定 Action 名称有效");
    AppBuilder::new().addon(
        AddonSpec::new(AddonName::new("bench").expect("固定 Addon 名称有效")).module(
            ModuleSpec::new(module).action(
                ActionSpec::new(
                    action,
                    RouteSpec::new(HttpMethod::Post, "/bench/echo", "bench.echo"),
                )
                .public(true),
                EchoAction,
            ),
        ),
    )
}

fn runtime_baseline(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("Tokio 基准运行时应创建成功");
    let tools = tools();
    let action: &dyn DynAction = &EchoAction;

    criterion.bench_function("action/typed_dispatch", |bencher| {
        bencher.iter(|| {
            let response = runtime
                .block_on(action.dispatch(context(&tools)))
                .expect("基准 Action 应成功");
            black_box(response)
        });
    });

    criterion.bench_function("params/body_extract", |bencher| {
        bencher.iter(|| {
            let mut request = Request::new(json!({ "message": "hello" }));
            let input =
                <EchoInput as ParamInput>::decode(&mut request).expect("固定输入应反序列化成功");
            black_box(input)
        });
    });

    criterion.bench_function("tools/direct_arc_access", |bencher| {
        let ctx = context(&tools);
        bencher.iter(|| black_box(ctx.tools() as *const Tools));
    });

    criterion.bench_function("tools/typed_extension_access", |bencher| {
        bencher.iter(|| black_box(tools.extension::<u64>().expect("固定工具应存在")));
    });

    let mut request_context = context(&tools);
    const BENCH_CONTEXT: yang_base::action::ContextKey<u64> =
        yang_base::context_key!("benchmark.value");
    request_context.request_context().insert(BENCH_CONTEXT, 7);
    criterion.bench_function("request_context/typed_access", |bencher| {
        bencher.iter(|| {
            black_box(
                *request_context
                    .request_context()
                    .get(BENCH_CONTEXT)
                    .expect("固定请求上下文应存在"),
            )
        });
    });

    let table = Table::new("org_user")
        .fields([
            Field::id("id"),
            Field::string("username", 64)
                .required()
                .filterable()
                .sortable(),
            Field::integer("status").filterable().sortable(),
        ])
        .build()
        .expect("基准表定义应有效");
    criterion.bench_function("tables/query_plan_build", |bencher| {
        bencher.iter(|| {
            let query = context(&tools)
                .with_table_definition(table.clone())
                .table_query()
                .expect("表查询应创建成功")
                .where_eq("status", json!(1))
                .expect("固定过滤应有效")
                .order_by("username", yang_base::table::SortOrder::Asc)
                .expect("固定排序应有效")
                .page(1, 20)
                .expect("固定分页应有效");
            black_box(query)
        });
    });

    let _runtime_guard = runtime.enter();
    let pool = MySqlPoolOptions::new()
        .connect_lazy("mysql://root:benchmark@127.0.0.1:3306/benchmark")
        .expect("lazy MySQL pool 应创建成功");
    let database =
        Database::from_pool(pool, DatabaseConfig::default()).expect("默认数据库配置应有效");
    let controlled_table = DbTableRef::new("org_user").expect("固定表引用有效");
    let controlled_id = DbFieldRef::new("id").expect("固定字段引用有效");
    let controlled_username = DbFieldRef::new("username").expect("固定字段引用有效");
    let controlled_status = DbFieldRef::new("status").expect("固定字段引用有效");
    criterion.bench_function("db/table_where_order_select_sql", |bencher| {
        bencher.iter(|| {
            let sql = database
                .table(&controlled_table)
                .field(&controlled_id)
                .field(&controlled_username)
                .where_and(&controlled_status, CompareOp::Eq, 1_i64)
                .order(&controlled_username, DbSortOrder::Asc)
                .limit(20)
                .try_to_sql()
                .expect("固定受控查询应生成 SQL");
            black_box(sql)
        });
    });

    let built = app_builder()
        .build(Arc::clone(&tools))
        .expect("固定 App 定义应构建成功");
    let reference = ActionRef::new(
        ModuleName::new("bench.echo").expect("固定 Module 名称有效"),
        ActionName::new("echo").expect("固定 Action 名称有效"),
    );
    criterion.bench_function("definition/registry_resolve", |bencher| {
        bencher.iter(|| {
            black_box(
                built
                    .registry()
                    .resolve(&reference)
                    .expect("构建期引用应解析为 slot"),
            )
        });
    });

    let typed = built
        .registry()
        .resolve_typed::<EchoInput, EchoOutput>(&reference)
        .expect("固定强类型引用应解析为 slot");
    criterion.bench_function("action/internal_typed_call", |bencher| {
        bencher.iter(|| {
            black_box(
                runtime
                    .block_on(built.registry().call(
                        typed,
                        context(&tools),
                        EchoInput {
                            message: "hello".to_string(),
                        },
                    ))
                    .expect("强类型内部调用应成功"),
            )
        });
    });

    criterion.bench_function("action/json_value_round_trip", |bencher| {
        bencher.iter(|| {
            black_box(
                runtime
                    .block_on(action.dispatch(context(&tools)))
                    .expect("JSON 边界调用应成功"),
            )
        });
    });

    criterion.bench_function("definition/app_build", |bencher| {
        bencher.iter(|| {
            black_box(
                app_builder()
                    .build(Arc::clone(&tools))
                    .expect("固定定义应构建成功"),
            )
        });
    });
}

criterion_group!(benches, runtime_baseline);
criterion_main!(benches);
