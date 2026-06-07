use std::path::PathBuf;
use std::{env, process};

fn main() {
    let workspace_root = match workspace_root() {
        Ok(workspace_root) => workspace_root,
        Err(error) => {
            eprintln!("i18n check failed: {error}");
            process::exit(1);
        }
    };
    let report = warp_i18n::check::check_workspace(workspace_root);

    if report.is_success() {
        println!(
            "i18n check passed: {} catalogs, {} Rust files, {} literal keys",
            report.catalog_count, report.rust_file_count, report.literal_key_count
        );
        return;
    }

    eprintln!("i18n check failed with {} error(s):", report.errors.len());
    for error in report.errors {
        eprintln!("- {error}");
    }
    process::exit(1);
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let root = match args.next() {
        Some(arg) => PathBuf::from(arg),
        None => env::current_dir()?,
    };

    Ok(warp_i18n::check::find_workspace_root(root)?)
}
