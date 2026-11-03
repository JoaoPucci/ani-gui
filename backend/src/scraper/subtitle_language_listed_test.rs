//! A provider's own `lang` field, shape by shape.

use super::listed_tag;

/// A field shaped like a tag stands in a tag's casing: a language, then
/// a script, then a region — two letters or three digits — with `_` or
/// `-` between.
#[test]
fn a_tag_shaped_field_stands_in_a_tags_casing() {
    assert_eq!(listed_tag("en", ""), "en");
    assert_eq!(listed_tag("FIL", ""), "fil");
    assert_eq!(listed_tag("pt_br", ""), "pt-BR");
    assert_eq!(listed_tag("es-419", ""), "es-419");
    assert_eq!(listed_tag("zh_hant_tw", ""), "zh-Hant-TW");
    assert_eq!(listed_tag(" sr-LATN ", ""), "sr-Latn");
}

/// A field that only starts like a tag is not one, and is read as a
/// name, then by the label, then as `und`.
#[test]
fn a_field_that_only_starts_like_a_tag_is_read_as_a_name() {
    // A region twice, a script after a region, a script twice.
    assert_eq!(listed_tag("pt-BR-PT", "Portuguese"), "pt");
    assert_eq!(
        listed_tag("zh-TW-Hant", "Chinese (- Traditional)"),
        "zh-Hant"
    );
    assert_eq!(listed_tag("zh-Hant-Hans", ""), "und");
    // Subtags of no known shape.
    assert_eq!(listed_tag("en-x", "English"), "en");
    assert_eq!(listed_tag("en-12", ""), "und");
    assert_eq!(listed_tag("e1", ""), "und");
    assert_eq!(listed_tag("english", "Arabic"), "en");
}
