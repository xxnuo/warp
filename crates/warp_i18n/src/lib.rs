use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::sync::{OnceLock, RwLock};

pub const AUTO_LOCALE: &str = "auto";
pub const FALLBACK_LOCALE: &str = "en-US";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocaleDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub native_name: &'static str,
    pub direction: TextDirection,
}

pub const SUPPORTED_LOCALES: &[LocaleDescriptor] = &[
    LocaleDescriptor {
        id: "en-US",
        name: "English (United States)",
        native_name: "English (United States)",
        direction: TextDirection::LeftToRight,
    },
    LocaleDescriptor {
        id: "zh-CN",
        name: "Chinese (Simplified)",
        native_name: "简体中文",
        direction: TextDirection::LeftToRight,
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locale {
    id: &'static str,
    requested: String,
}

impl Locale {
    pub fn id(&self) -> &'static str {
        self.id
    }

    pub fn requested(&self) -> &str {
        &self.requested
    }

    pub fn descriptor(&self) -> LocaleDescriptor {
        descriptor(self.id).unwrap_or_else(|| descriptor(FALLBACK_LOCALE).unwrap())
    }

    pub fn direction(&self) -> TextDirection {
        self.descriptor().direction
    }
}

type Catalog = BTreeMap<String, String>;

static ACTIVE_LOCALE: OnceLock<RwLock<Locale>> = OnceLock::new();
static EN_US: OnceLock<Catalog> = OnceLock::new();
static ZH_CN: OnceLock<Catalog> = OnceLock::new();

pub fn init(locale: impl AsRef<str>) -> Locale {
    set_locale(locale)
}

pub fn init_from_environment() -> Locale {
    if let Ok(locale) = env::var("WARP_LOCALE") {
        if !locale.trim().is_empty() {
            return set_locale(locale);
        }
    }
    set_locale(AUTO_LOCALE)
}

pub fn set_locale(locale: impl AsRef<str>) -> Locale {
    let locale = resolve_locale(locale.as_ref());
    let mut active = active_locale_lock().write().expect("locale lock poisoned");
    *active = locale.clone();
    locale
}

pub fn active_locale() -> Locale {
    active_locale_lock()
        .read()
        .expect("locale lock poisoned")
        .clone()
}

pub fn descriptor(locale: &str) -> Option<LocaleDescriptor> {
    SUPPORTED_LOCALES
        .iter()
        .copied()
        .find(|supported| supported.id == locale)
}

pub fn supported_locale_ids() -> impl Iterator<Item = &'static str> {
    SUPPORTED_LOCALES.iter().map(|locale| locale.id)
}

pub fn is_supported(locale: &str) -> bool {
    canonical_locale_id(locale).is_some()
}

pub fn resolve_locale(requested: &str) -> Locale {
    let requested = requested.trim();
    let requested = if requested.is_empty() {
        AUTO_LOCALE
    } else {
        requested
    };
    let resolved = if requested.eq_ignore_ascii_case(AUTO_LOCALE) {
        system_locale()
            .as_deref()
            .and_then(canonical_locale_id)
            .unwrap_or(FALLBACK_LOCALE)
    } else {
        canonical_locale_id(requested).unwrap_or(FALLBACK_LOCALE)
    };
    Locale {
        id: resolved,
        requested: requested.to_owned(),
    }
}

pub fn tr(key: &str) -> String {
    tr_with(key, &[])
}

pub fn tr_with(key: &str, args: &[(&str, &str)]) -> String {
    let raw = lookup(key).unwrap_or(key);
    if args.is_empty() {
        return raw.to_owned();
    }

    let mut translated = raw.to_owned();
    for (name, value) in args {
        translated = translated.replace(&format!("{{{name}}}"), value);
    }
    translated
}

pub fn missing_keys(locale: &str) -> Vec<String> {
    let Some(catalog) = catalog(locale) else {
        return fallback_catalog().keys().cloned().collect();
    };
    fallback_catalog()
        .keys()
        .filter(|key| !catalog.contains_key(*key))
        .cloned()
        .collect()
}

pub fn extra_keys(locale: &str) -> Vec<String> {
    let Some(catalog) = catalog(locale) else {
        return Vec::new();
    };
    catalog
        .keys()
        .filter(|key| !fallback_catalog().contains_key(*key))
        .cloned()
        .collect()
}

pub fn catalog_keys(locale: &str) -> BTreeSet<String> {
    catalog(locale)
        .map(|catalog| catalog.keys().cloned().collect())
        .unwrap_or_default()
}

fn active_locale_lock() -> &'static RwLock<Locale> {
    ACTIVE_LOCALE.get_or_init(|| RwLock::new(resolve_locale(AUTO_LOCALE)))
}

fn lookup(key: &str) -> Option<&'static str> {
    let locale = active_locale();
    catalog(locale.id())
        .and_then(|catalog| catalog.get(key))
        .or_else(|| fallback_catalog().get(key))
        .map(String::as_str)
}

fn catalog(locale: &str) -> Option<&'static Catalog> {
    match canonical_locale_id(locale)? {
        "en-US" => Some(fallback_catalog()),
        "zh-CN" => Some(ZH_CN.get_or_init(|| parse_catalog(include_str!("../locales/zh-CN.json")))),
        _ => None,
    }
}

fn fallback_catalog() -> &'static Catalog {
    EN_US.get_or_init(|| parse_catalog(include_str!("../locales/en-US.json")))
}

fn parse_catalog(source: &str) -> Catalog {
    serde_json::from_str(source).expect("locale catalog must be valid JSON")
}

fn canonical_locale_id(locale: &str) -> Option<&'static str> {
    let normalized = locale
        .trim()
        .split('.')
        .next()
        .unwrap_or_default()
        .replace('_', "-")
        .to_ascii_lowercase();

    match normalized.as_str() {
        "en" | "en-us" | "en-gb" | "en-au" | "en-ca" | "en-nz" | "en-ie" | "en-za" => Some("en-US"),
        "zh" | "zh-cn" | "zh-hans" | "zh-hans-cn" | "zh-sg" | "zh-my" => Some("zh-CN"),
        _ => None,
    }
}

#[cfg(not(target_family = "wasm"))]
fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

#[cfg(target_family = "wasm")]
fn system_locale() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCALE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn canonicalizes_supported_locales() {
        assert_eq!(resolve_locale("en_GB.UTF-8").id(), "en-US");
        assert_eq!(resolve_locale("zh-Hans-CN").id(), "zh-CN");
        assert_eq!(resolve_locale("fr-FR").id(), "en-US");
    }

    #[test]
    fn translates_active_locale() {
        let _guard = TEST_LOCALE_LOCK.lock().unwrap();
        set_locale("zh-CN");
        assert_eq!(tr("common.next"), "下一步");

        set_locale("en-US");
        assert_eq!(tr("common.next"), "Next");
    }

    #[test]
    fn interpolates_values() {
        let _guard = TEST_LOCALE_LOCK.lock().unwrap();
        set_locale("en-US");
        assert_eq!(
            tr_with("onboarding.model.starting_at", &[("price", "$20")]),
            "Starting at $20/mo"
        );
    }

    #[test]
    fn falls_back_to_key_for_unknown_messages() {
        let _guard = TEST_LOCALE_LOCK.lock().unwrap();
        set_locale("zh-CN");
        assert_eq!(tr("missing.key"), "missing.key");
    }

    #[test]
    fn locale_catalogs_match_fallback_keys() {
        for locale in supported_locale_ids().filter(|locale| *locale != FALLBACK_LOCALE) {
            assert_eq!(missing_keys(locale), Vec::<String>::new(), "{locale}");
            assert_eq!(extra_keys(locale), Vec::<String>::new(), "{locale}");
        }
    }
}
