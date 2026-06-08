use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use super::*;

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn new(name: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("warp_i18n_{name}_{timestamp}"));
        fs::create_dir_all(path.join("crates/warp_i18n/locales")).unwrap();
        fs::create_dir_all(path.join("app/src")).unwrap();
        fs::create_dir_all(path.join("crates/onboarding/src")).unwrap();
        fs::create_dir_all(path.join("crates/ui_components/src")).unwrap();
        fs::write(path.join("crates/warp_i18n/locales.json"), registry()).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn registry() -> &'static str {
    r#"{
  "fallback": "en-US",
  "locales": [
    {
      "id": "en-US",
      "name": "English (United States)",
      "native_name": "English (United States)",
      "direction": "ltr",
      "aliases": ["en", "en-US"]
    },
    {
      "id": "zh-CN",
      "name": "Chinese (Simplified)",
      "native_name": "Chinese (Simplified)",
      "direction": "ltr",
      "aliases": ["zh", "zh-CN"]
    }
  ]
}"#
}

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn extracts_placeholders() {
    assert_eq!(
        placeholders("Hello {name}, {count} items and {{literal}}"),
        set(&["count", "name"])
    );
}

#[test]
fn extracts_literal_translation_keys() {
    let source = r#"
        use warp_i18n::{tr, tr_with};

        const STATIC_KEY: &str = "common.static";

        fn main() {
            let _ = tr("common.next");
            let _ = tr(STATIC_KEY);
            let _ = tr_with("workspace.title", &[("title", title)]);
            let _ = tr(dynamic_key);
            let _ = serde_json::from_str::<String>("{}");
        }
    "#;

    assert_eq!(
        literal_tr_keys(source),
        set(&["common.next", "common.static", "workspace.title"])
    );
}

#[test]
fn extracts_literal_translation_call_args() {
    let source = r#"
        use warp_i18n::{tr, tr_with};

        fn main() {
            let _ = tr("common.next");
            let _ = tr_with("workspace.title", &[("title", title), ("count", &count)]);
            let _ = tr_with("workspace.dynamic", args);
        }
    "#;

    let calls = literal_tr_calls(source);
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].args, TranslationArgs::None);
    assert_eq!(
        calls[1].args,
        TranslationArgs::Static(set(&["count", "title"]))
    );
    assert_eq!(calls[2].args, TranslationArgs::Dynamic);
}

#[test]
fn ignores_translation_lookalikes_outside_code() {
    let source = r##"
        fn main() {
            let _ = "tr(\"string.literal\")";
            let _ = b"tr(\"byte.string\")";
            let _ = c"tr(\"c.string\")";
            let _ = r#"tr("raw.string")"#;
            let _ = br#"tr("byte.raw.string")"#;
            let _ = cr#"tr("c.raw.string")"#;
            // tr("line.comment")
            /*
                tr("block.comment")
            */
            let _ = tr("common.next");
        }
    "##;

    assert_eq!(literal_tr_keys(source), set(&["common.next"]));
}

#[test]
fn check_workspace_accepts_complete_fixture() {
    let workspace = TempWorkspace::new("complete");
    write(
        &workspace.path().join("crates/warp_i18n/locales/en-US.json"),
        r#"{
  "common.count": "{count} items",
  "common.next": "Next",
  "workflow.placeholder": "{{name}}"
}"#,
    );
    write(
        &workspace.path().join("crates/warp_i18n/locales/zh-CN.json"),
        r#"{
  "common.count": "{count} 个项目",
  "common.next": "下一步",
  "workflow.placeholder": "{{name}}"
}"#,
    );
    write(
        &workspace.path().join("app/src/main.rs"),
        r#"
use warp_i18n::{tr, tr_with};

fn main() {
    let _ = tr("common.next");
    let _ = tr_with("common.count", &[("count", "1")]);
}
"#,
    );

    let report = check_workspace(workspace.path());
    assert_eq!(report.errors, Vec::<String>::new());
    assert_eq!(report.catalog_count, 2);
    assert_eq!(report.literal_key_count, 2);
}

#[test]
fn check_workspace_reports_drift() {
    let workspace = TempWorkspace::new("drift");
    write(
        &workspace.path().join("crates/warp_i18n/locales/en-US.json"),
        r#"{
  "common.count": "{count} items",
  "common.next": "Next"
}"#,
    );
    write(
        &workspace.path().join("crates/warp_i18n/locales/zh-CN.json"),
        r#"{
  "common.count": "{total} 个项目"
}"#,
    );
    write(
        &workspace.path().join("app/src/main.rs"),
        r#"
use warp_i18n::tr;

fn main() {
    let _ = tr("common.missing");
}
"#,
    );

    let report = check_workspace(workspace.path());
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("zh-CN is missing catalog key common.next")));
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("zh-CN:common.count placeholders differ")));
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("source references missing i18n key common.missing")));
}

#[test]
fn check_workspace_reports_source_arg_drift() {
    let workspace = TempWorkspace::new("arg_drift");
    write(
        &workspace.path().join("crates/warp_i18n/locales/en-US.json"),
        r#"{
  "common.count": "{count} items",
  "common.next": "Next"
}"#,
    );
    write(
        &workspace.path().join("crates/warp_i18n/locales/zh-CN.json"),
        r#"{
  "common.count": "{count} 个项目",
  "common.next": "下一步"
}"#,
    );
    write(
        &workspace.path().join("app/src/main.rs"),
        r#"
use warp_i18n::{tr, tr_with};

fn main() {
    let _ = tr("common.count");
    let _ = tr_with("common.count", &[("total", "1")]);
    let _ = tr_with("common.next", &[("count", "1")]);
    let _ = tr_with("common.count", dynamic_args);
}
"#,
    );

    let report = check_workspace(workspace.path());
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("common.count requires i18n args [count], but tr was used")));
    assert!(report.errors.iter().any(|error| error
        .contains("common.count i18n args differ from catalog: expected [count], got [total]")));
    assert!(report.errors.iter().any(|error| {
        error.contains("common.next i18n args differ from catalog: expected [], got [count]")
    }));
    assert!(report.errors.iter().any(|error| error
        .contains("common.count i18n args are dynamic and cannot be checked against [count]")));
}

#[test]
fn check_workspace_discovers_i18n_crates() {
    let workspace = TempWorkspace::new("discovery");
    write(
        &workspace.path().join("crates/warp_i18n/locales/en-US.json"),
        r#"{
  "common.next": "Next"
}"#,
    );
    write(
        &workspace.path().join("crates/warp_i18n/locales/zh-CN.json"),
        r#"{
  "common.next": "下一步"
}"#,
    );
    write(
        &workspace.path().join("crates/new_ui/Cargo.toml"),
        r#"[package]
name = "new_ui"
version = "0.1.0"
edition = "2021"

[dependencies]
warp_i18n.workspace = true
"#,
    );
    write(
        &workspace.path().join("crates/new_ui/src/lib.rs"),
        r#"
pub fn render() {
    let _ = warp_i18n::tr("new_ui.title");
}
"#,
    );

    let report = check_workspace(workspace.path());
    assert!(report
        .errors
        .iter()
        .any(|error| error.contains("source references missing i18n key new_ui.title")));
}
