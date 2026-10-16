//! Rule metadata stored in comments, in the format of the Roundcube
//! *managesieve* plugin (concept §3, decision D4):
//!
//! ```sieve
//! # rule:[Taxes]
//! if header :contains "from" "@tax-office.example" { … }
//!
//! # rule:[Disabled rule]
//! if false # header :contains "subject" "x"
//! { … }
//! ```

use crate::ast::{Comment, CommentKind};

const NAME_PREFIX: &str = "rule:[";

/// Extracts the rule name from a `# rule:[Name]` comment.
pub fn parse_rule_name(comment: &Comment) -> Option<String> {
    if comment.kind != CommentKind::Hash {
        return None;
    }
    let rest = comment.text.trim_start().strip_prefix(NAME_PREFIX)?;
    let name = rest.trim_end().strip_suffix(']')?;
    // Roundcube trims names, both when reading and when saving.
    Some(name.trim().to_owned())
}

/// The comment text (without `#`) for a rule name.
pub fn format_rule_name(name: &str) -> String {
    // A line break would end the comment and turn the rest into code.
    let name: String = name.chars().map(|c| if c == '\r' || c == '\n' { ' ' } else { c }).collect();
    format!(" {NAME_PREFIX}{}]", name.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    fn hash(text: &str) -> Comment {
        Comment { text: text.into(), kind: CommentKind::Hash, span: Span::default() }
    }

    #[test]
    fn names() {
        assert_eq!(parse_rule_name(&hash(" rule:[Taxes]")).as_deref(), Some("Taxes"));
        assert_eq!(parse_rule_name(&hash("rule:[a [b] c]")).as_deref(), Some("a [b] c"));
        assert_eq!(parse_rule_name(&hash(" rule:[]")).as_deref(), Some(""));
        assert_eq!(parse_rule_name(&hash(" rule:[open")), None);
        assert_eq!(parse_rule_name(&hash(" other comment")), None);
        let bracket = Comment { kind: CommentKind::Bracket, ..hash(" rule:[x]") };
        assert_eq!(parse_rule_name(&bracket), None);
        assert_eq!(format_rule_name("a\nb"), " rule:[a b]");
        assert_eq!(parse_rule_name(&hash(&format_rule_name("Ä [x] #"))).as_deref(), Some("Ä [x] #"));
    }
}
