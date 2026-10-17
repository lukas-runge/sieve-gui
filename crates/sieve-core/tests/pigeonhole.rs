//! Parses every script of the Dovecot Pigeonhole test suite (LGPL-2.1, used
//! only as external test input, never vendored) and checks the round trip.
//!
//! Run with: `PIGEONHOLE_DIR=/path/to/pigeonhole cargo test -p sieve-core --test pigeonhole -- --ignored`
//! (see `scripts/fetch-pigeonhole.sh`).

mod common;

use std::path::{Path, PathBuf};

use sieve_core::{parse, PrintOptions};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "svtest" || e == "sieve") {
            out.push(path);
        }
    }
}

#[test]
#[ignore = "needs PIGEONHOLE_DIR"]
fn pigeonhole_suite_parses_and_roundtrips() {
    // Relative paths are resolved against the workspace root.
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = workspace.join(std::env::var("PIGEONHOLE_DIR").expect("set PIGEONHOLE_DIR"));
    let mut files = Vec::new();
    collect(&root.join("tests"), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no test scripts found under {}", root.display());

    let options = PrintOptions::default();
    let mut failures = Vec::new();
    let (mut parsed, mut expected_errors, mut not_utf8) = (0, 0, 0);
    for path in &files {
        let rel = path.strip_prefix(&root).unwrap().display().to_string();
        let Ok(src) = String::from_utf8(std::fs::read(path).unwrap()) else {
            not_utf8 += 1;
            continue;
        };
        // Scripts under `errors` and `recover` directories are meant to fail;
        // most of them are still syntactically valid.
        let may_fail = rel.contains("/errors") || rel.contains("/recover/");
        match parse(&src) {
            Ok(_) => {
                parsed += 1;
                common::roundtrip(&src, &options);
                common::roundtrip(&src, &PrintOptions::readable());
            }
            Err(e) if may_fail => {
                expected_errors += 1;
                eprintln!("expected failure: {rel}: {e}");
            }
            Err(e) => failures.push(format!("{rel}: {e}")),
        }
    }
    eprintln!(
        "{} files: {parsed} parsed, {expected_errors} failed in errors/ dirs, {not_utf8} not UTF-8",
        files.len()
    );
    assert!(failures.is_empty(), "unexpected parse failures:\n{}", failures.join("\n"));
}
