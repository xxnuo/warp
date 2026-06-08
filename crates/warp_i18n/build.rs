use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct LocaleRegistry {
    fallback: String,
    locales: Vec<LocaleRegistration>,
}

#[derive(Debug, Deserialize)]
struct LocaleRegistration {
    id: String,
    name: String,
    native_name: String,
    direction: String,
    #[serde(default)]
    aliases: Vec<String>,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let registry_path = manifest_dir.join("locales.json");
    println!("cargo:rerun-if-changed={}", registry_path.display());

    let registry_source = fs::read_to_string(&registry_path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", registry_path.display());
    });
    let registry: LocaleRegistry = serde_json::from_str(&registry_source).unwrap_or_else(|err| {
        panic!("failed to parse {}: {err}", registry_path.display());
    });
    validate_registry(&manifest_dir, &registry);

    for locale in &registry.locales {
        println!(
            "cargo:rerun-if-changed={}",
            manifest_dir
                .join("locales")
                .join(format!("{}.json", locale.id))
                .display()
        );
    }

    let generated = generated_source(&registry);
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("locale_registry.rs"), generated).unwrap();
}

fn validate_registry(manifest_dir: &Path, registry: &LocaleRegistry) {
    assert!(
        !registry.fallback.trim().is_empty(),
        "fallback locale is empty"
    );
    assert!(!registry.locales.is_empty(), "locale registry is empty");

    let mut ids = BTreeSet::new();
    let mut aliases = BTreeMap::new();
    let mut has_fallback = false;

    for locale in &registry.locales {
        assert!(!locale.id.trim().is_empty(), "locale id is empty");
        assert!(
            !locale.name.trim().is_empty(),
            "{} name is empty",
            locale.id
        );
        assert!(
            !locale.native_name.trim().is_empty(),
            "{} native_name is empty",
            locale.id
        );
        assert!(
            matches!(locale.direction.as_str(), "ltr" | "rtl"),
            "{} direction must be ltr or rtl",
            locale.id
        );
        assert!(
            ids.insert(locale.id.clone()),
            "duplicate locale id {}",
            locale.id
        );
        if locale.id == registry.fallback {
            has_fallback = true;
        }

        let locale_path = manifest_dir
            .join("locales")
            .join(format!("{}.json", locale.id));
        assert!(
            locale_path.is_file(),
            "{} is listed but {} does not exist",
            locale.id,
            locale_path.display()
        );

        for alias in locale_aliases(locale) {
            let normalized = normalize_locale_id(&alias);
            if let Some(existing) = aliases.insert(normalized.clone(), locale.id.clone()) {
                panic!(
                    "locale alias {alias} normalizes to {normalized}, already owned by {existing}"
                );
            }
        }
    }

    assert!(
        has_fallback,
        "fallback locale {} is not registered",
        registry.fallback
    );
}

fn generated_source(registry: &LocaleRegistry) -> String {
    let mut source = String::new();
    source.push_str(&format!(
        "pub const FALLBACK_LOCALE: &str = {};\n\n",
        rust_string(&registry.fallback)
    ));
    source.push_str("pub const SUPPORTED_LOCALES: &[LocaleDescriptor] = &[\n");
    for locale in &registry.locales {
        source.push_str("    LocaleDescriptor {\n");
        source.push_str(&format!("        id: {},\n", rust_string(&locale.id)));
        source.push_str(&format!("        name: {},\n", rust_string(&locale.name)));
        source.push_str(&format!(
            "        native_name: {},\n",
            rust_string(&locale.native_name)
        ));
        source.push_str(&format!(
            "        direction: {},\n",
            direction_variant(&locale.direction)
        ));
        source.push_str("    },\n");
    }
    source.push_str("];\n\n");

    source.push_str(&format!(
        "static CATALOG_SOURCES: [CatalogSource; {}] = [\n",
        registry.locales.len()
    ));
    for locale in &registry.locales {
        source.push_str(&format!(
            "    CatalogSource {{ id: {}, source: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/locales/{}.json\")), catalog: OnceLock::new() }},\n",
            rust_string(&locale.id),
            locale.id
        ));
    }
    source.push_str("];\n\n");

    source.push_str("fn canonical_locale_id(locale: &str) -> Option<&'static str> {\n");
    source.push_str("    let normalized = normalize_locale_id(locale);\n");
    source.push_str("    match normalized.as_str() {\n");
    for locale in &registry.locales {
        let aliases = locale_aliases(locale)
            .into_iter()
            .map(|alias| normalize_locale_id(&alias))
            .collect::<BTreeSet<_>>();
        let pattern = aliases
            .iter()
            .map(|alias| rust_string(alias))
            .collect::<Vec<_>>()
            .join(" | ");
        source.push_str(&format!(
            "        {pattern} => Some({}),\n",
            rust_string(&locale.id)
        ));
    }
    source.push_str("        _ => None,\n");
    source.push_str("    }\n");
    source.push_str("}\n");
    source
}

fn locale_aliases(locale: &LocaleRegistration) -> BTreeSet<String> {
    let mut aliases = BTreeSet::from([locale.id.clone()]);
    aliases.extend(locale.aliases.iter().cloned());
    aliases
}

fn direction_variant(direction: &str) -> &'static str {
    match direction {
        "ltr" => "TextDirection::LeftToRight",
        "rtl" => "TextDirection::RightToLeft",
        _ => unreachable!(),
    }
}

fn normalize_locale_id(locale: &str) -> String {
    locale
        .trim()
        .split('.')
        .next()
        .unwrap_or_default()
        .replace('_', "-")
        .to_ascii_lowercase()
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}
