//! The value hls.js reads after an attribute's `=`.

use super::*;

#[test]
fn a_quoted_value_runs_to_a_quote_before_a_comma_or_the_end() {
    assert_eq!(attribute_value("\"data:,x\",B=1"), "data:,x");
    assert_eq!(attribute_value("\"a\"b\""), "a\"b");
    assert_eq!(attribute_value("plain,B=1"), "plain");
    assert_eq!(attribute_value("\"open,B=1"), "\"open");
}
