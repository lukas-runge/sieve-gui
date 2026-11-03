//! Scripts Synology MailPlus keeps on the server (see
//! fixtures/mailplus/README.md): a setup sieve-gui didn't create and must
//! include unchanged.

use std::path::Path;

use sieve_core::root::{parse_root, root_script, script_links};
use sieve_core::{import, PrintOptions};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mailplus").join(name);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn synomc_includes_the_other_scripts_and_files_messages() {
    let links = script_links(&fixture("synomc.sieve"));
    // `include :global "spam_rewrite_subject"` is no personal script.
    assert_eq!(links.includes, ["spam_rule", "filter"]);
    assert!(links.files_messages);
    for name in ["filter.sieve", "spam_rule.sieve"] {
        let links = script_links(&fixture(name));
        assert!(links.includes.is_empty() && !links.files_messages, "{name}: {links:?}");
    }
}

#[test]
fn synomc_is_not_a_sieve_gui_root() {
    assert_eq!(parse_root(&fixture("synomc.sieve")), None);
}

#[test]
fn scripts_import() {
    for name in ["synomc.sieve", "filter.sieve", "spam_rule.sieve"] {
        import(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn root_keeps_synomc_unchanged() {
    let scripts = ["synomc".to_owned(), "sort".to_owned()];
    let text = root_script(&scripts, &scripts[..1], true, &PrintOptions::readable());
    let root = parse_root(&text).unwrap();
    assert_eq!(root.scripts, scripts);
    assert_eq!(root.unchanged, ["synomc"]);
}
