pub fn is_japanese_char(c: char) -> bool {
    matches!(
        c,
        '\u{3040}'..='\u{30FF}'   // Hiragana & Katakana
        | '\u{31F0}'..='\u{31FF}' // Katakana Phonetic Extensions
        | '\u{FF65}'..='\u{FF9F}' // Halfwidth Katakana
        | '\u{4E00}'..='\u{9FAF}' // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
    )
}

pub fn contains_japanese(text: &str) -> bool {
    text.chars().any(is_japanese_char)
}

pub fn count_japanese_chars(text: &str) -> usize {
    text.chars().filter(|&c| is_japanese_char(c)).count()
}

pub fn clean_for_dedup(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
