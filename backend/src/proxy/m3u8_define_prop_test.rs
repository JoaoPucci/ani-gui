//! Properties over the pieces `EXT-X-DEFINE` is read and spelled with,
//! mounted by `#[path]` beside the module.

use super::parse::{attributes, percent_decode};
use super::spell::{spell, spell_attributes};
use super::Variables;
use proptest::prelude::*;

proptest! {
    /// Any value a quoted string can carry, every byte percent-encoded,
    /// decodes back to itself; a decoded `"`, CR or LF decodes to
    /// nothing.
    #[test]
    fn a_percent_encoded_value_decodes_back_unless_a_quoted_string_cannot_carry_it(
        value in "[ !#-~]{0,24}",
        bad in prop_oneof![Just(b'"'), Just(b'\r'), Just(b'\n')],
    ) {
        let encode = |bytes: &[u8]| bytes.iter().map(|b| format!("%{b:02X}")).collect::<String>();
        prop_assert_eq!(percent_decode(&encode(value.as_bytes())), Some(value.clone()));
        let mut with_bad = value.into_bytes();
        with_bad.push(bad);
        prop_assert_eq!(percent_decode(&encode(&with_bad)), None);
    }

    /// An attribute list written with quoted and unquoted values reads
    /// back as the names and values written.
    #[test]
    fn an_attribute_list_reads_back_as_written(
        pairs in proptest::collection::btree_map("[A-Z][A-Z-]{0,8}", ("[ !#-~]{0,12}", proptest::bool::ANY), 0..6),
    ) {
        let list: Vec<String> = pairs
            .iter()
            .map(|(name, (value, quoted))| {
                if *quoted { format!("{name}=\"{value}\"") } else { format!("{name}={}", value.replace([',', ' ', '"'], "")) }
            })
            .collect();
        let read = attributes(&list.join(","));
        for (name, (value, quoted)) in &pairs {
            let expected = if *quoted { value.clone() } else { value.replace([',', ' ', '"'], "") };
            prop_assert_eq!(read.get(name), Some(&expected), "{}", list.join(","));
        }
    }

    /// Spelling leaves text with no reference as it is, spells a
    /// reference to its value, and reads no inserted value for
    /// references; a tag's unquoted value is spelled only when it spells
    /// a hexadecimal sequence, and left as written otherwise.
    #[test]
    fn spelling_leaves_plain_text_alone_and_reads_no_value_twice(
        text in "[ -#%-z|~]{0,32}",
        name in "[a-z]{1,6}",
        value in "[ -~]{0,16}",
    ) {
        let vars = Variables::from([(name.clone(), value.clone())]);
        prop_assert_eq!(spell(&text, &vars), text.clone());
        prop_assert_eq!(spell(&format!("a{{${name}}}b"), &vars), format!("a{value}b"));
        let nested = Variables::from([(name.clone(), format!("{{${name}}}"))]);
        prop_assert_eq!(spell(&format!("{{${name}}}"), &nested), format!("{{${name}}}"));
        let line = format!("#EXT-X-KEY:METHOD={{${name}}},URI=\"k\"");
        let hex = value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
            .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_hexdigit()));
        let spelled = spell_attributes(&line, &vars);
        if hex {
            prop_assert_eq!(spelled, format!("#EXT-X-KEY:METHOD={value},URI=\"k\""));
        } else {
            prop_assert_eq!(spelled, line);
        }
    }
}
