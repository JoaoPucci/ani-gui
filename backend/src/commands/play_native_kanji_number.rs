//! Japanese numbers written in kanji, compound ones included — 十 is
//! 10, 十二 12, 二十一 21, 百 100 — read where the title grammar
//! expects a Japanese number: after 第 and before a counter. Split
//! from `play_native_title_grammar` for the per-file complexity bar.

const DIGITS: &str = "〇一二三四五六七八九";

fn is_numeral(c: char) -> bool {
    DIGITS.contains(c) || c == '十' || c == '百'
}

/// The number a run of kanji numerals writes the compound way — a
/// digit before 百, a digit before 十, a last digit, each optional and
/// in that order: "十" is 10, "十二" 12, "二十一" 21, "百二" 102, "八"
/// 8, and "〇" alone 0. `None` for any other run ("一二", "十十") and
/// for anything that is not kanji numerals.
pub(super) fn kanji_number(text: &str) -> Option<u32> {
    if text == "〇" {
        return Some(0);
    }
    let digit = |c: char| {
        DIGITS
            .chars()
            .position(|k| k == c)
            .filter(|&d| d > 0)
            .map(|d| d as u32)
    };
    let mut chars = text.chars().peekable();
    let mut total = 0;
    for (unit, value) in [('百', 100), ('十', 10)] {
        let mut ahead = chars.clone();
        let lead = ahead.next().and_then(digit);
        if lead.is_some() && ahead.peek() == Some(&unit) {
            chars = ahead;
        }
        if chars.peek() == Some(&unit) {
            chars.next();
            total += lead.unwrap_or(1) * value;
        }
    }
    if let Some(c) = chars.next() {
        total += digit(c)?;
    }
    (chars.next().is_none() && total > 0).then_some(total)
}

/// `text` with every run of kanji numerals that stands where a
/// Japanese number is expected — right after 第, or right before a
/// counter (号, 期, 話, 部, クール) — written in ASCII digits:
/// "怪獣八号" reads "怪獣8号", "第十二部" "第12部". Any other run is
/// left as it is, so the 一 of "一騎当千" is no number.
pub(super) fn with_kanji_numbers(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_numeral(chars[i]) {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let end = (i..chars.len())
            .find(|&j| !is_numeral(chars[j]))
            .unwrap_or(chars.len());
        let run: String = chars[i..end].iter().collect();
        let after_dai = i > 0 && chars[i - 1] == '第';
        let rest: String = chars[end..].iter().collect();
        let before_counter = ["号", "期", "話", "部", "クール"]
            .iter()
            .any(|c| rest.starts_with(c));
        match kanji_number(&run).filter(|_| after_dai || before_counter) {
            Some(n) => out.push_str(&n.to_string()),
            None => out.push_str(&run),
        }
        i = end;
    }
    out
}

#[cfg(test)]
#[path = "play_native_kanji_number_test.rs"]
mod tests;
