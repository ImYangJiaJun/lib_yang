use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;
use yang_runtime::config::{
    ConfigSources, EnvironmentBinding, EnvironmentValueKind, SecretBinding,
};
use yang_runtime::shutdown::ShutdownBudget;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct TestSettings {
    app: AppSettings,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct AppSettings {
    name: String,
    workers: u64,
}

const ENVIRONMENT: &[EnvironmentBinding] = &[
    EnvironmentBinding::new("TEST_APP_NAME", "app", "name", EnvironmentValueKind::Text),
    EnvironmentBinding::new(
        "TEST_APP_WORKERS",
        "app",
        "workers",
        EnvironmentValueKind::Integer,
    ),
];

const SECRETS: &[SecretBinding] = &[];

#[test]
fn typed_config_sources_apply_declared_environment_overrides_only() {
    let sources = ConfigSources::new("TEST_", "TEST_SECRET_DIR", ENVIRONMENT, SECRETS);
    let environment = BTreeMap::from([
        ("TEST_APP_NAME".to_string(), "runtime-service".to_string()),
        ("TEST_APP_WORKERS".to_string(), "4".to_string()),
    ]);

    let settings: TestSettings = sources
        .parse_with_sources(
            "[app]\nname = \"file-service\"\nworkers = 1\n",
            &environment,
            None,
        )
        .unwrap_or_else(|error| panic!("声明过的覆盖应成功: {error}"));

    assert_eq!(
        settings,
        TestSettings {
            app: AppSettings {
                name: "runtime-service".to_string(),
                workers: 4,
            },
        }
    );
}

#[tokio::test]
async fn shutdown_phases_share_one_absolute_deadline() {
    let budget = ShutdownBudget::new(Duration::from_millis(120));
    budget.begin("test").await;
    budget
        .run_phase("first", async {
            tokio::time::sleep(Duration::from_millis(80)).await;
            Ok(())
        })
        .await
        .unwrap_or_else(|error| panic!("第一阶段应完成: {error}"));

    assert!(budget
        .run_phase("second", async {
            tokio::time::sleep(Duration::from_millis(80)).await;
            Ok(())
        })
        .await
        .is_err());
}
