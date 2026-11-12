//! Run with scripts/test-native-ui.sh inside its isolated session bus/display.
use blockwork_core::{
    accessibility::{self, Query},
    config,
    input::{
        ui::{UiAction, UiScope, UiSelector},
        value::Evaluated,
    },
};
use std::time::{Duration, Instant};
fn selector(app: &str, name: &str) -> UiSelector {
    UiSelector {
        scope: UiScope::App,
        app: app.into(),
        name: name.into(),
        ..UiSelector::default()
    }
}
#[test]
#[ignore = "requires the isolated GTK or Qt fixture; use scripts/test-native-ui.sh"]
fn native_ui_actions_queries_and_events_are_live() {
    assert_eq!(
        std::env::var("BLOCKWORK_UI_FIXTURE").as_deref(),
        Ok("gtk"),
        "run scripts/test-native-ui.sh"
    );
    let directory = std::env::temp_dir().join(format!("blockwork-atspi-{}", uuid::Uuid::new_v4()));
    config::set_config_dir_override(directory.clone());
    config::save_settings(&config::AppSettings {
        ui_automation_enabled: Some(true),
        ..Default::default()
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let app = loop {
        let applications = accessibility::applications().unwrap_or_default();
        if let Some(app) = applications
            .into_iter()
            .find(|app| app.to_lowercase().contains("native-ui") || app.contains("fixture"))
        {
            break app;
        }
        assert!(
            Instant::now() < deadline,
            "fixture didn't register on AT-SPI"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    let button = selector(&app, "Activate");
    assert_eq!(
        accessibility::query(&button, Query::Exists).unwrap(),
        Evaluated::Bool(true)
    );
    accessibility::action(&button, UiAction::Invoke, "").unwrap();
    let status = selector(&app, "Status");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if accessibility::query(&status, Query::Text).unwrap() == Evaluated::Text("pressed".into())
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let input = selector(&app, "Input");
    accessibility::action(&input, UiAction::SetText, "updated").unwrap();
    assert_eq!(
        accessibility::query(&input, Query::Text).unwrap(),
        Evaluated::Text("updated".into())
    );
    accessibility::action(&input, UiAction::Focus, "").unwrap();
    let amount = selector(&app, "Amount");
    accessibility::action(&amount, UiAction::SetValue, "3.5").unwrap();
    assert_eq!(
        accessibility::query(&amount, Query::Value).unwrap(),
        Evaluated::Number(3.5)
    );
    assert!(accessibility::action(&amount, UiAction::SetValue, "20").is_err());
    assert!(accessibility::action(&amount, UiAction::SetValue, "NaN").is_err());
    assert!(accessibility::action(&selector(&app, "Duplicate"), UiAction::Invoke, "").is_err());
    assert_eq!(
        accessibility::query(&selector(&app, "Disabled"), Query::Enabled).unwrap(),
        Evaluated::Bool(false)
    );
    assert!(accessibility::action(&selector(&app, "Disabled"), UiAction::Invoke, "").is_err());
    assert!(accessibility::query(&selector(&app, "Secret"), Query::Text).is_err());
    assert!(
        !serde_json::to_string(
            &accessibility::inspect(&UiSelector {
                scope: UiScope::App,
                app: app.clone(),
                ..Default::default()
            })
            .unwrap()
        )
        .unwrap()
        .contains("protected-fixture-value")
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let before = loop {
        match accessibility::event_revision() {
            Ok(revision) => break revision,
            Err(error) => {
                assert!(Instant::now() < deadline, "{error}");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    accessibility::action(&input, UiAction::SetText, "event update").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while accessibility::event_revision().unwrap() <= before {
        assert!(Instant::now() < deadline, "no accessibility event arrived");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::fs::remove_dir_all(directory).unwrap();
}
