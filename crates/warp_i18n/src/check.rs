use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use crate::{supported_locale_ids, FALLBACK_LOCALE};

pub const DEFAULT_SOURCE_DIRS: &[&str] = &[
    "app/src",
    "crates/onboarding/src",
    "crates/ui_components/src",
];

type Catalog = BTreeMap<String, String>;

#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub workspace_root: PathBuf,
    pub source_dirs: Vec<PathBuf>,
}

impl CheckOptions {
    pub fn for_workspace_root(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            source_dirs: DEFAULT_SOURCE_DIRS.iter().map(PathBuf::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckReport {
    pub errors: Vec<String>,
    pub catalog_count: usize,
    pub rust_file_count: usize,
    pub literal_key_count: usize,
}

impl CheckReport {
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug)]
pub enum CheckError {
    Io(io::Error),
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CheckError {}

impl From<io::Error> for CheckError {
    fn from(error: io::Error) -> Self {
        CheckError::Io(error)
    }
}

pub fn find_workspace_root(start: impl AsRef<Path>) -> Result<PathBuf, CheckError> {
    let mut current = fs::canonicalize(start)?;
    loop {
        if current.join("Cargo.toml").is_file() && current.join("crates/warp_i18n").is_dir() {
            return Ok(current);
        }
        if !current.pop() {
            return Err(
                io::Error::new(io::ErrorKind::NotFound, "could not find workspace root").into(),
            );
        }
    }
}

pub fn check_workspace(workspace_root: impl Into<PathBuf>) -> CheckReport {
    check(CheckOptions::for_workspace_root(workspace_root))
}

pub fn check(options: CheckOptions) -> CheckReport {
    let mut report = CheckReport::default();
    let locale_dir = options.workspace_root.join("crates/warp_i18n/locales");
    let catalogs = load_catalogs(&options.workspace_root, &locale_dir, &mut report);
    report.catalog_count = catalogs.len();

    check_catalog_files(&catalogs, &mut report);
    check_catalog_keys(&catalogs, &mut report);
    check_placeholders(&catalogs, &mut report);
    if should_check_runtime_catalogs(&options.workspace_root) {
        check_runtime_catalogs(&catalogs, &mut report);
    }
    check_source_keys(&options, &catalogs, &mut report);

    report
}

fn load_catalogs(
    workspace_root: &Path,
    locale_dir: &Path,
    report: &mut CheckReport,
) -> BTreeMap<String, Catalog> {
    let mut catalogs = BTreeMap::new();
    let entries = match fs::read_dir(locale_dir) {
        Ok(entries) => entries,
        Err(error) => {
            report.errors.push(format!(
                "{}: failed to read locale directory: {error}",
                display_path(workspace_root, locale_dir)
            ));
            return catalogs;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                report
                    .errors
                    .push(format!("failed to read locale directory entry: {error}"));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Some(locale) = path.file_stem().and_then(|stem| stem.to_str()) else {
            report.errors.push(format!(
                "{}: locale file name must be valid UTF-8",
                display_path(workspace_root, &path)
            ));
            continue;
        };
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                report.errors.push(format!(
                    "{}: failed to read locale file: {error}",
                    display_path(workspace_root, &path)
                ));
                continue;
            }
        };
        match serde_json::from_str::<Catalog>(&source) {
            Ok(catalog) => {
                catalogs.insert(locale.to_owned(), catalog);
            }
            Err(error) => {
                report.errors.push(format!(
                    "{}: invalid locale JSON: {error}",
                    display_path(workspace_root, &path)
                ));
            }
        }
    }

    catalogs
}

fn check_catalog_files(catalogs: &BTreeMap<String, Catalog>, report: &mut CheckReport) {
    let supported: BTreeSet<String> = supported_locale_ids().map(str::to_owned).collect();
    let files: BTreeSet<String> = catalogs.keys().cloned().collect();

    if !files.contains(FALLBACK_LOCALE) {
        report.errors.push(format!(
            "missing fallback locale file {FALLBACK_LOCALE}.json"
        ));
    }

    for locale in supported.difference(&files) {
        report
            .errors
            .push(format!("missing locale file {locale}.json"));
    }

    for locale in files.difference(&supported) {
        report.errors.push(format!(
            "locale file {locale}.json is not listed in SUPPORTED_LOCALES"
        ));
    }
}

fn check_catalog_keys(catalogs: &BTreeMap<String, Catalog>, report: &mut CheckReport) {
    let Some(fallback) = catalogs.get(FALLBACK_LOCALE) else {
        return;
    };
    let fallback_keys: BTreeSet<&String> = fallback.keys().collect();

    for (locale, catalog) in catalogs {
        for (key, value) in catalog {
            if value.trim().is_empty() {
                report
                    .errors
                    .push(format!("{locale}:{key} has an empty translation"));
            }
        }

        if locale == FALLBACK_LOCALE {
            continue;
        }

        let keys: BTreeSet<&String> = catalog.keys().collect();
        for key in fallback_keys.difference(&keys) {
            report
                .errors
                .push(format!("{locale} is missing catalog key {key}"));
        }
        for key in keys.difference(&fallback_keys) {
            report
                .errors
                .push(format!("{locale} has extra catalog key {key}"));
        }
    }
}

fn check_placeholders(catalogs: &BTreeMap<String, Catalog>, report: &mut CheckReport) {
    let Some(fallback) = catalogs.get(FALLBACK_LOCALE) else {
        return;
    };

    for (locale, catalog) in catalogs {
        if locale == FALLBACK_LOCALE {
            continue;
        }

        for (key, fallback_value) in fallback {
            let Some(value) = catalog.get(key) else {
                continue;
            };
            let fallback_placeholders = placeholders(fallback_value);
            let locale_placeholders = placeholders(value);
            if fallback_placeholders != locale_placeholders {
                report.errors.push(format!(
                    "{locale}:{key} placeholders differ from {FALLBACK_LOCALE}: expected {}, got {}",
                    format_set(&fallback_placeholders),
                    format_set(&locale_placeholders)
                ));
            }
        }
    }
}

fn check_runtime_catalogs(catalogs: &BTreeMap<String, Catalog>, report: &mut CheckReport) {
    for locale in supported_locale_ids() {
        let Some(catalog) = catalogs.get(locale) else {
            continue;
        };
        let runtime_keys = crate::catalog_keys(locale);
        let file_keys: BTreeSet<String> = catalog.keys().cloned().collect();
        if runtime_keys != file_keys {
            report.errors.push(format!(
                "runtime catalog for {locale} does not match {locale}.json"
            ));
        }
    }
}

fn should_check_runtime_catalogs(workspace_root: &Path) -> bool {
    let checked_locale_dir = fs::canonicalize(workspace_root.join("crates/warp_i18n/locales"));
    let compiled_locale_dir =
        fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("locales"));

    checked_locale_dir.ok() == compiled_locale_dir.ok()
}

fn check_source_keys(
    options: &CheckOptions,
    catalogs: &BTreeMap<String, Catalog>,
    report: &mut CheckReport,
) {
    let Some(fallback) = catalogs.get(FALLBACK_LOCALE) else {
        return;
    };
    let mut literal_keys = BTreeSet::new();
    let source_dirs = source_dirs(&options.workspace_root, &options.source_dirs);

    for source_dir in source_dirs {
        let path = options.workspace_root.join(source_dir);
        for path in collect_rust_files(&options.workspace_root, &path, report) {
            report.rust_file_count += 1;
            let source = match fs::read_to_string(&path) {
                Ok(source) => source,
                Err(error) => {
                    report.errors.push(format!(
                        "{}: failed to read Rust source: {error}",
                        display_path(&options.workspace_root, &path)
                    ));
                    continue;
                }
            };
            for key in literal_tr_keys(&source) {
                literal_keys.insert(key);
            }
        }
    }

    report.literal_key_count = literal_keys.len();
    for key in literal_keys {
        if !fallback.contains_key(&key) {
            report
                .errors
                .push(format!("source references missing i18n key {key}"));
        }
    }
}

fn source_dirs(workspace_root: &Path, configured: &[PathBuf]) -> BTreeSet<PathBuf> {
    let mut source_dirs: BTreeSet<PathBuf> = configured.iter().cloned().collect();
    source_dirs.extend(discover_i18n_source_dirs(workspace_root));
    source_dirs
}

fn discover_i18n_source_dirs(workspace_root: &Path) -> BTreeSet<PathBuf> {
    let mut source_dirs = BTreeSet::new();
    collect_manifest_source_dir(
        workspace_root,
        Path::new("app/Cargo.toml"),
        &mut source_dirs,
    );

    let crates_dir = workspace_root.join("crates");
    let Ok(entries) = fs::read_dir(&crates_dir) else {
        return source_dirs;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || path.file_name().and_then(|name| name.to_str()) == Some("warp_i18n") {
            continue;
        }
        if let Ok(relative) = path.join("Cargo.toml").strip_prefix(workspace_root) {
            collect_manifest_source_dir(workspace_root, relative, &mut source_dirs);
        }
    }

    source_dirs
}

fn collect_manifest_source_dir(
    workspace_root: &Path,
    manifest_path: &Path,
    source_dirs: &mut BTreeSet<PathBuf>,
) {
    let manifest = workspace_root.join(manifest_path);
    let Ok(contents) = fs::read_to_string(&manifest) else {
        return;
    };
    if !contents.contains("warp_i18n") {
        return;
    }

    let Some(package_dir) = manifest_path.parent() else {
        return;
    };
    let source_dir = package_dir.join("src");
    if workspace_root.join(&source_dir).is_dir() {
        source_dirs.insert(source_dir);
    }
}

fn collect_rust_files(
    workspace_root: &Path,
    path: &Path,
    report: &mut CheckReport,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            report.errors.push(format!(
                "{}: failed to read source directory: {error}",
                display_path(workspace_root, path)
            ));
            return files;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                report
                    .errors
                    .push(format!("failed to read source directory entry: {error}"));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_rust_files(workspace_root, &path, report));
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }

    files
}

pub fn placeholders(value: &str) -> BTreeSet<String> {
    let mut placeholders = BTreeSet::new();
    let chars: Vec<char> = value.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] != '{' {
            index += 1;
            continue;
        }
        if chars.get(index + 1) == Some(&'{') {
            index += 2;
            continue;
        }

        let start = index + 1;
        let mut end = start;
        while end < chars.len() && is_placeholder_char(chars[end]) {
            end += 1;
        }

        if end > start && chars.get(end) == Some(&'}') && is_placeholder_start(chars[start]) {
            placeholders.insert(chars[start..end].iter().collect());
            index = end + 1;
        } else {
            index += 1;
        }
    }

    placeholders
}

pub fn literal_tr_keys(source: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if !is_ident_start(chars[index]) {
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < chars.len() && is_ident_continue(chars[index]) {
            index += 1;
        }
        let ident: String = chars[start..index].iter().collect();
        if ident != "tr" && ident != "tr_with" {
            continue;
        }

        let mut cursor = skip_whitespace(&chars, index);
        if chars.get(cursor) != Some(&'(') {
            continue;
        }
        cursor = skip_whitespace(&chars, cursor + 1);
        if let Some((key, _end)) = parse_string_literal(&chars, cursor) {
            keys.insert(key);
        }
    }

    keys
}

fn parse_string_literal(chars: &[char], mut index: usize) -> Option<(String, usize)> {
    if chars.get(index) != Some(&'"') {
        return None;
    }
    index += 1;
    let mut value = String::new();

    while index < chars.len() {
        match chars[index] {
            '"' => return Some((value, index + 1)),
            '\\' => {
                let escaped = *chars.get(index + 1)?;
                match escaped {
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    'n' => value.push('\n'),
                    'r' => value.push('\r'),
                    't' => value.push('\t'),
                    _ => value.push(escaped),
                }
                index += 2;
            }
            ch => {
                value.push(ch);
                index += 1;
            }
        }
    }

    None
}

fn skip_whitespace(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() && chars[index].is_whitespace() {
        index += 1;
    }
    index
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_placeholder_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_placeholder_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn format_set(values: &BTreeSet<String>) -> String {
    if values.is_empty() {
        "[]".to_owned()
    } else {
        format!(
            "[{}]",
            values.iter().cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

fn display_path(workspace_root: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
