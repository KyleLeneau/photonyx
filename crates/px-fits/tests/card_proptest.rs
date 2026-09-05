//! ADR 006 P1-T4: property-based roundtrip test for `Card`. Scope, precisely:
//! *any `Card` we construct* serializes to exactly 80 ASCII bytes and
//! re-parses to an equal `Card` — not "any 80 bytes parse losslessly" (that's
//! not true and isn't the property being tested; `Value::Invalid` and
//! whitespace/case normalization are intentionally lossy on the parse side).
//!
//! Inputs are bounded so serialization never needs to truncate a comment to
//! fit in 80 bytes, which would break the roundtrip by construction rather
//! than by a bug.

use proptest::prelude::*;
use px_fits::card::{Card, Value};

/// Keywords: uppercase alnum/hyphen, 1-8 chars — stays clear of the
/// COMMENT/HISTORY/END/HIERARCH/CONTINUE/blank special-cased keywords and of
/// anything containing '=' or '/' that would be ambiguous with card syntax.
fn keyword_strategy() -> impl Strategy<Value = String> {
    "[A-Z][A-Z0-9-]{0,7}".prop_filter("not a reserved/special keyword", |k| {
        !matches!(
            k.as_str(),
            "COMMENT" | "HISTORY" | "END" | "HIERARCH" | "CONTINUE"
        )
    })
}

/// Comment text: printable ASCII, short enough that keyword, value, " / ",
/// and comment always fit in 80 bytes for the value shapes this test uses.
/// A `/` embedded in the comment is fine to include here (unlike in the
/// value token): `extract_comment` treats only the first `/` after the
/// value as the delimiter, so anything after it, slashes included, is
/// captured verbatim as comment text.
fn comment_strategy() -> impl Strategy<Value = Option<String>> {
    prop::option::of("[ -~]{1,30}".prop_map(|s| s.trim().to_string()))
        .prop_map(|o| o.filter(|s| !s.is_empty()))
}

/// String values: printable ASCII, no quote (escaping is covered by a
/// dedicated unit test in `src/card.rs`), short enough to leave room for a
/// keyword and comment.
fn string_value_strategy() -> impl Strategy<Value = String> {
    "[ -&(-~]{0,20}".prop_map(|s| s.trim_end().to_string())
}

fn value_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<bool>().prop_map(Value::Logical),
        any::<i32>().prop_map(|i| Value::Integer(i as i64)),
        (-1.0e10f64..1.0e10).prop_map(Value::Float),
        string_value_strategy().prop_map(Value::String),
        Just(Value::Undefined),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn card_roundtrips_through_bytes(
        keyword in keyword_strategy(),
        value in value_strategy(),
        comment in comment_strategy(),
    ) {
        let card = Card::new(keyword, value, comment);
        let bytes = card.to_bytes();
        prop_assert_eq!(bytes.len(), 80);
        prop_assert!(bytes.iter().all(|b| b.is_ascii()));

        let reparsed = Card::parse(&bytes);
        prop_assert_eq!(reparsed, card);
    }
}
