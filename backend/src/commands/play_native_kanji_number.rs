//! Japanese numbers written in kanji, compound ones included — 十 is
//! 10, 十二 12, 二十一 21, 百 100 — read where the title grammar
//! expects a Japanese number: after 第 and before a counter. Split
//! from `play_native_title_grammar` for the per-file complexity bar.

const DIGITS: &str = "〇一二三四五六七八九";

fn is_numeral(c: char) -> bool {
    DIGITS.contains(c) || c == '十' || c == '百'
}

/// The number a run of kanji numerals writes, or `None` when `text`
/// is anything else or writes nothing: "十" is 10, "十二" 12, "二十一"
/// 21, "百二" 102, "八" 8.
pub(super) fn kanji_number(text: &str) -> Option<u32> {
    let (mut total, mut digit) = (0u32, None::<u32>);
    for c in text.chars() {
        if let Some(d) = DIGITS.chars().position(|k| k == c) {
            digit = Some(d as u32);
        } else if c == '十' || c == '百' {
            let unit = if c == '十' { 10 } else { 100 };
            total += digit.take().unwrap_or(1) * unit;
        } else {
            return None;
        }
    }
    let n = total + digit.unwrap_or(0);
    (n > 0).then_some(n)
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
        let before_counter = chars.get(end).is_some_and(|c| "号期話部ク".contains(*c));
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
