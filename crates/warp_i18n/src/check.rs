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

const LOCALE_REGISTRY_PATH: &str = "crates/warp_i18n/locales.json";

#[derive(Clone, Debug, serde::Deserialize)]
struct LocaleRegistry {
    fallback: String,
    locales: Vec<LocaleRegistration>,
}

impl LocaleRegistry {
    fn locale_ids(&self) -> BTreeSet<String> {
        self.locales
            .iter()
            .map(|locale| locale.id.clone())
            .collect()
    }

    fn fallback(&self) -> &str {
        self.fallback.as_str()
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
struct LocaleRegistration {
    id: String,
    name: String,
    native_name: String,
    direction: String,
    #[serde(default)]
    aliases: Vec<String>,
}

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
    let registry = load_registry(&options.workspace_root, &mut report);
    let fallback_locale = registry
        .as_ref()
        .map(LocaleRegistry::fallback)
        .unwrap_or(FALLBACK_LOCALE)
        .to_owned();
    let locale_dir = options.workspace_root.join("crates/warp_i18n/locales");
    let catalogs = load_catalogs(&options.workspace_root, &locale_dir, &mut report);
    report.catalog_count = catalogs.len();

    check_catalog_files(registry.as_ref(), &catalogs, &mut report);
    check_catalog_keys(&fallback_locale, &catalogs, &mut report);
    check_placeholders(&fallback_locale, &catalogs, &mut report);
    if should_check_runtime_catalogs(&options.workspace_root) {
        check_runtime_catalogs(registry.as_ref(), &catalogs, &mut report);
    }
    check_source_keys(&options, &catalogs, &fallback_locale, &mut report);

    report
}

fn load_registry(workspace_root: &Path, report: &mut CheckReport) -> Option<LocaleRegistry> {
    let path = workspace_root.join(LOCALE_REGISTRY_PATH);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            report.errors.push(format!(
                "{}: failed to read locale registry: {error}",
                display_path(workspace_root, &path)
            ));
            return None;
        }
    };
    let registry = match serde_json::from_str::<LocaleRegistry>(&source) {
        Ok(registry) => registry,
        Err(error) => {
            report.errors.push(format!(
                "{}: invalid locale registry JSON: {error}",
                display_path(workspace_root, &path)
            ));
            return None;
        }
    };

    check_registry(workspace_root, &registry, report);
    Some(registry)
}

fn check_registry(workspace_root: &Path, registry: &LocaleRegistry, report: &mut CheckReport) {
    if registry.fallback.trim().is_empty() {
        report
            .errors
            .push("locale registry fallback is empty".to_owned());
    }
    if registry.locales.is_empty() {
        report
            .errors
            .push("locale registry has no locales".to_owned());
    }

    let mut ids = BTreeSet::new();
    let mut aliases = BTreeMap::new();
    let mut has_fallback = false;

    for locale in &registry.locales {
        if locale.id.trim().is_empty() {
            report
                .errors
                .push("locale registry has an empty id".to_owned());
            continue;
        }
        if locale.name.trim().is_empty() {
            report
                .errors
                .push(format!("locale registry {} has an empty name", locale.id));
        }
        if locale.native_name.trim().is_empty() {
            report.errors.push(format!(
                "locale registry {} has an empty native_name",
                locale.id
            ));
        }
        if !matches!(locale.direction.as_str(), "ltr" | "rtl") {
            report.errors.push(format!(
                "locale registry {} direction must be ltr or rtl",
                locale.id
            ));
        }
        if !ids.insert(locale.id.clone()) {
            report.errors.push(format!(
                "locale registry has duplicate locale {}",
                locale.id
            ));
        }
        if locale.id == registry.fallback {
            has_fallback = true;
        }

        for alias in locale_aliases(locale) {
            let normalized = normalize_locale_id(&alias);
            if let Some(existing) = aliases.insert(normalized.clone(), locale.id.clone()) {
                report.errors.push(format!(
                    "locale registry alias {alias} normalizes to {normalized}, already owned by {existing}"
                ));
            }
        }

        let locale_path = workspace_root
            .join("crates/warp_i18n/locales")
            .join(format!("{}.json", locale.id));
        if !locale_path.is_file() {
            report.errors.push(format!(
                "locale registry lists {}, but {} does not exist",
                locale.id,
                display_path(workspace_root, &locale_path)
            ));
        }
    }

    if !registry.fallback.trim().is_empty() && !has_fallback {
        report.errors.push(format!(
            "locale registry fallback {} is not registered",
            registry.fallback
        ));
    }
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

fn check_catalog_files(
    registry: Option<&LocaleRegistry>,
    catalogs: &BTreeMap<String, Catalog>,
    report: &mut CheckReport,
) {
    let supported: BTreeSet<String> = registry
        .map(LocaleRegistry::locale_ids)
        .unwrap_or_else(|| supported_locale_ids().map(str::to_owned).collect());
    let fallback_locale = registry
        .map(LocaleRegistry::fallback)
        .unwrap_or(FALLBACK_LOCALE);
    let files: BTreeSet<String> = catalogs.keys().cloned().collect();

    if !files.contains(fallback_locale) {
        report.errors.push(format!(
            "missing fallback locale file {fallback_locale}.json"
        ));
    }

    for locale in supported.difference(&files) {
        report
            .errors
            .push(format!("missing locale file {locale}.json"));
    }

    for locale in files.difference(&supported) {
        report.errors.push(format!(
            "locale file {locale}.json is not listed in {LOCALE_REGISTRY_PATH}"
        ));
    }
}

fn check_catalog_keys(
    fallback_locale: &str,
    catalogs: &BTreeMap<String, Catalog>,
    report: &mut CheckReport,
) {
    let Some(fallback) = catalogs.get(fallback_locale) else {
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

        if locale == fallback_locale {
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

fn check_placeholders(
    fallback_locale: &str,
    catalogs: &BTreeMap<String, Catalog>,
    report: &mut CheckReport,
) {
    let Some(fallback) = catalogs.get(fallback_locale) else {
        return;
    };

    for (locale, catalog) in catalogs {
        if locale == fallback_locale {
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
                    "{locale}:{key} placeholders differ from {fallback_locale}: expected {}, got {}",
                    format_set(&fallback_placeholders),
                    format_set(&locale_placeholders)
                ));
            }
        }
    }
}

fn check_runtime_catalogs(
    registry: Option<&LocaleRegistry>,
    catalogs: &BTreeMap<String, Catalog>,
    report: &mut CheckReport,
) {
    if let Some(registry) = registry {
        let runtime_ids: BTreeSet<String> = supported_locale_ids().map(str::to_owned).collect();
        let registry_ids = registry.locale_ids();
        if runtime_ids != registry_ids {
            report.errors.push(format!(
                "runtime supported locales differ from {LOCALE_REGISTRY_PATH}"
            ));
        }
        if FALLBACK_LOCALE != registry.fallback() {
            report.errors.push(format!(
                "runtime fallback locale {FALLBACK_LOCALE} differs from {}",
                registry.fallback()
            ));
        }
    }

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
    fallback_locale: &str,
    report: &mut CheckReport,
) {
    let Some(fallback) = catalogs.get(fallback_locale) else {
        return;
    };
    let mut literal_keys = BTreeSet::new();
    let mut reported_missing_keys = BTreeSet::new();
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
            for call in literal_tr_calls(&source) {
                literal_keys.insert(call.key.clone());
                let source_location = format!(
                    "{}:{}",
                    display_path(&options.workspace_root, &path),
                    call.line
                );

                let Some(message) = fallback.get(&call.key) else {
                    if reported_missing_keys.insert(call.key.clone()) {
                        report.errors.push(format!(
                            "{source_location}: source references missing i18n key {}",
                            call.key
                        ));
                    }
                    continue;
                };

                let expected_args = placeholders(message);
                match call.args {
                    TranslationArgs::None => {
                        if !expected_args.is_empty() {
                            report.errors.push(format!(
                                "{source_location}: {} requires i18n args {}, but tr was used",
                                call.key,
                                format_set(&expected_args)
                            ));
                        }
                    }
                    TranslationArgs::Static(actual_args) => {
                        if actual_args != expected_args {
                            report.errors.push(format!(
                                "{source_location}: {} i18n args differ from catalog: expected {}, got {}",
                                call.key,
                                format_set(&expected_args),
                                format_set(&actual_args)
                            ));
                        }
                    }
                    TranslationArgs::Dynamic => {
                        if !expected_args.is_empty() {
                            report.errors.push(format!(
                                "{source_location}: {} i18n args are dynamic and cannot be checked against {}",
                                call.key,
                                format_set(&expected_args)
                            ));
                        }
                    }
                }
            }
        }
    }

    report.literal_key_count = literal_keys.len();
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationCall {
    pub key: String,
    pub args: TranslationArgs,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranslationArgs {
    None,
    Static(BTreeSet<String>),
    Dynamic,
}

pub fn literal_tr_keys(source: &str) -> BTreeSet<String> {
    literal_tr_calls(source)
        .into_iter()
        .map(|call| call.key)
        .collect()
}

pub fn literal_tr_calls(source: &str) -> Vec<TranslationCall> {
    let mut calls = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let constants = string_constants(&chars);
    let mut index = 0;

    while index < chars.len() {
        if let Some(next_index) = skip_non_code(&chars, index) {
            index = next_index;
            continue;
        }
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
        let Some(call_end) = find_matching_delimiter(&chars, cursor, '(', ')') else {
            continue;
        };
        cursor = skip_whitespace(&chars, cursor + 1);
        let Some((key, key_end)) = parse_key_arg(&chars, cursor, &constants) else {
            continue;
        };

        let args = if ident == "tr_with" {
            parse_translation_args(&chars, key_end, call_end)
        } else {
            TranslationArgs::None
        };
        calls.push(TranslationCall {
            key,
            args,
            line: line_number(&chars, start),
        });
        index = call_end + 1;
    }

    calls
}

fn string_constants(chars: &[char]) -> BTreeMap<String, String> {
    let mut constants = BTreeMap::new();
    let mut index = 0;

    while index < chars.len() {
        if let Some(next_index) = skip_non_code(chars, index) {
            index = next_index;
            continue;
        }
        let Some((ident, ident_end)) = parse_identifier(chars, index) else {
            index += 1;
            continue;
        };
        index = ident_end;
        if ident != "const" && ident != "static" {
            continue;
        }

        let name_start = skip_whitespace(chars, index);
        let Some((name, name_end)) = parse_identifier(chars, name_start) else {
            continue;
        };
        let Some(eq_index) = find_char_until(chars, name_end, '=') else {
            continue;
        };
        let value_start = skip_whitespace(chars, eq_index + 1);
        if let Some((value, value_end)) = parse_string_literal(chars, value_start) {
            constants.insert(name, value);
            index = value_end;
        }
    }

    constants
}

fn parse_key_arg(
    chars: &[char],
    index: usize,
    constants: &BTreeMap<String, String>,
) -> Option<(String, usize)> {
    if let Some((key, end)) = parse_string_literal(chars, index) {
        return Some((key, end));
    }
    let (ident, end) = parse_identifier(chars, index)?;
    constants.get(&ident).cloned().map(|key| (key, end))
}

fn parse_translation_args(chars: &[char], key_end: usize, call_end: usize) -> TranslationArgs {
    let Some(comma) = find_top_level_comma(chars, key_end, call_end) else {
        return TranslationArgs::Dynamic;
    };
    let mut cursor = skip_whitespace(chars, comma + 1);
    if chars.get(cursor) == Some(&'&') {
        cursor = skip_whitespace(chars, cursor + 1);
    }
    if chars.get(cursor) != Some(&'[') {
        return TranslationArgs::Dynamic;
    }
    let Some(args_end) = find_matching_delimiter(chars, cursor, '[', ']') else {
        return TranslationArgs::Dynamic;
    };

    let mut args = BTreeSet::new();
    let mut index = cursor + 1;
    while index < args_end {
        if chars[index] != '(' {
            index += 1;
            continue;
        }
        let name_start = skip_whitespace(chars, index + 1);
        let Some((name, name_end)) = parse_string_literal(chars, name_start) else {
            index += 1;
            continue;
        };
        let after_name = skip_whitespace(chars, name_end);
        if chars.get(after_name) == Some(&',') {
            args.insert(name);
        }
        index = name_end;
    }

    TranslationArgs::Static(args)
}

fn parse_identifier(chars: &[char], mut index: usize) -> Option<(String, usize)> {
    if !chars.get(index).copied().is_some_and(is_ident_start) {
        return None;
    }
    let start = index;
    index += 1;
    while index < chars.len() && is_ident_continue(chars[index]) {
        index += 1;
    }
    Some((chars[start..index].iter().collect(), index))
}

fn find_char_until(chars: &[char], mut index: usize, needle: char) -> Option<usize> {
    while index < chars.len() {
        match chars[index] {
            ch if ch == needle => return Some(index),
            ';' | '\n' => return None,
            '"' => {
                index = parse_string_literal(chars, index)?.1;
            }
            _ => index += 1,
        }
    }
    None
}

fn find_top_level_comma(chars: &[char], mut index: usize, end: usize) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    while index < end {
        match chars[index] {
            '"' => {
                index = parse_string_literal(chars, index)?.1;
                continue;
            }
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ',' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                return Some(index);
            }
            _ => {}
        }
        index += 1;
    }

    None
}

fn find_matching_delimiter(
    chars: &[char],
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    if chars.get(open_index) != Some(&open) {
        return None;
    }

    let mut depth = 1usize;
    let mut index = open_index + 1;
    while index < chars.len() {
        match chars[index] {
            '"' => {
                index = parse_string_literal(chars, index)?.1;
                continue;
            }
            ch if ch == open => depth += 1,
            ch if ch == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }

    None
}

fn line_number(chars: &[char], index: usize) -> usize {
    chars[..index].iter().filter(|ch| **ch == '\n').count() + 1
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

fn skip_non_code(chars: &[char], index: usize) -> Option<usize> {
    match (chars.get(index), chars.get(index + 1)) {
        (Some('/'), Some('/')) => Some(skip_line_comment(chars, index + 2)),
        (Some('/'), Some('*')) => Some(skip_block_comment(chars, index + 2)?),
        (Some('"'), _) => Some(parse_string_literal(chars, index)?.1),
        (Some('b' | 'c'), Some('"')) => Some(parse_string_literal(chars, index + 1)?.1),
        (Some('b' | 'c'), Some('r')) => parse_raw_string_literal(chars, index + 1),
        (Some('r'), _) => parse_raw_string_literal(chars, index),
        _ => None,
    }
}

fn skip_line_comment(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() && chars[index] != '\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(chars: &[char], mut index: usize) -> Option<usize> {
    let mut depth = 1usize;
    while index + 1 < chars.len() {
        match (chars[index], chars[index + 1]) {
            ('/', '*') => {
                depth += 1;
                index += 2;
            }
            ('*', '/') => {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => index += 1,
        }
    }
    None
}

fn parse_raw_string_literal(chars: &[char], index: usize) -> Option<usize> {
    if chars.get(index) != Some(&'r') {
        return None;
    }
    let mut cursor = index + 1;
    let mut hashes = 0usize;
    while chars.get(cursor) == Some(&'#') {
        hashes += 1;
        cursor += 1;
    }
    if chars.get(cursor) != Some(&'"') {
        return None;
    }
    cursor += 1;

    while cursor < chars.len() {
        if chars[cursor] != '"' {
            cursor += 1;
            continue;
        }
        let mut hash_cursor = cursor + 1;
        let mut matched = 0usize;
        while matched < hashes && chars.get(hash_cursor) == Some(&'#') {
            matched += 1;
            hash_cursor += 1;
        }
        if matched == hashes {
            return Some(hash_cursor);
        }
        cursor += 1;
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

fn locale_aliases(locale: &LocaleRegistration) -> BTreeSet<String> {
    let mut aliases = BTreeSet::from([locale.id.clone()]);
    aliases.extend(locale.aliases.iter().cloned());
    aliases
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
