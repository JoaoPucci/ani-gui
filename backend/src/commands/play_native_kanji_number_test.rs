use super::*;

#[test]
fn compound_kanji_numbers_read_as_their_value() {
    for (text, n) in [
        ("一", 1),
        ("八", 8),
        ("十", 10),
        ("十二", 12),
        ("二十", 20),
        ("二十一", 21),
        ("百", 100),
        ("百二", 102),
    ] {
        assert_eq!(kanji_number(text), Some(n), "{text}");
    }
    assert_eq!(kanji_number("十a"), None);
    assert_eq!(kanji_number(""), None);
}

#[test]
fn only_a_number_where_one_is_expected_is_rewritten() {
    assert_eq!(with_kanji_numbers("ショー第十二部"), "ショー第12部");
    assert_eq!(with_kanji_numbers("怪獣八号"), "怪獣8号");
    assert_eq!(with_kanji_numbers("第二十一話"), "第21話");
    assert_eq!(with_kanji_numbers("一騎当千"), "一騎当千");
    assert_eq!(with_kanji_numbers("ショー 十"), "ショー 十");
}

#[test]
fn only_the_compound_way_writes_a_number_and_zero_is_zero() {
    for text in ["一二", "十十", "二二十", "百百", "十百", "一〇"] {
        assert_eq!(kanji_number(text), None, "{text}");
    }
    assert_eq!(kanji_number("〇"), Some(0));
    assert_eq!(with_kanji_numbers("第〇期"), "第0期");
    assert_eq!(with_kanji_numbers("一クラス"), "一クラス");
    assert_eq!(with_kanji_numbers("第二クール"), "第2クール");
}
