use sieve_core::ast::ClearSpans;
use sieve_core::{parse, print_script, PrintOptions};

/// Asserts that printing `src` and parsing it again gives the same tree, and
/// that printing is idempotent.
pub fn roundtrip(src: &str, options: &PrintOptions) {
    let mut original = parse(src).unwrap_or_else(|e| panic!("{e}\n{src}"));
    let printed = print_script(&original, options);
    let mut reparsed = parse(&printed).unwrap_or_else(|e| panic!("{e}\nprinted:\n{printed}"));
    assert_eq!(print_script(&reparsed, options), printed, "printing is not idempotent");
    original.clear_spans();
    reparsed.clear_spans();
    assert_eq!(original, reparsed, "tree changed after printing:\n{printed}");
}
