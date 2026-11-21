// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Broad Tokyo Japanese IPA. Long vowels and small-tsu consonants use the
//! length mark. Pitch accent and narrow vowel marks are removed.

/// IPA for a dictionary headword. The particles は, へ, and を use their
/// spoken forms when the headword itself is that particle.
pub fn ipa_for(word: &str, reading: &str) -> String {
    let word_key = kana_key(word);
    let reading_key = kana_key(reading);
    let raw = if word_key == "は" && reading_key == "は" {
        "wa".to_owned()
    } else if word_key == "へ" && reading_key == "へ" {
        "e".to_owned()
    } else {
        from_kana(reading)
    };
    broaden(&raw)
}

/// Broad Tokyo IPA: official letters, length marks, and no pitch or narrowing.
pub fn broaden(text: &str) -> String {
    let mut cleaned = String::new();
    for character in text.chars() {
        if dropped_from_broad_ipa(character) {
            continue;
        }
        cleaned.push(match character {
            'g' => 'ɡ',
            'r' => 'ɾ',
            other => other,
        });
    }
    collapse_lengths(&cleaned)
}

fn dropped_from_broad_ipa(character: char) -> bool {
    if character.is_whitespace()
        || matches!(
            character,
            '/' | '[' | ']' | '(' | ')' | '.' | ',' | '|' | 'ˈ' | 'ˌ' | 'ˑ'
        )
    {
        return true;
    }
    let code = character as u32;
    (0x0300..=0x036F).contains(&code)
        || (0x02B0..=0x02B8).contains(&code)
        || (0xA700..=0xA71F).contains(&code)
}

fn collapse_lengths(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        let character = chars[index];
        if character == 'ː' {
            if !output.ends_with('ː') {
                output.push('ː');
            }
            index += 1;
            continue;
        }
        if chars.get(index + 1) == Some(&character) {
            output.push(character);
            output.push('ː');
            index += 2;
            continue;
        }
        output.push(character);
        index += 1;
    }
    output
}

/// Hiragana/katakana letters only, in hiragana, for comparisons.
pub fn kana_key(text: &str) -> String {
    to_hiragana(text)
        .chars()
        .filter(|character| is_hiragana(*character))
        .collect()
}

fn from_kana(reading: &str) -> String {
    let converted = to_hiragana(reading);
    let chars: Vec<char> = converted
        .chars()
        .filter(|character| !matches!(character, ' ' | '　' | '・' | '·' | '.' | '．'))
        .collect();
    let mut index = 0;
    let mut output = String::new();
    let mut geminate = false;
    while index < chars.len() {
        let character = chars[index];
        if character == 'っ' {
            geminate = true;
            index += 1;
            continue;
        }
        if character == 'ー' {
            push_length(&mut output);
            index += 1;
            continue;
        }
        if character == 'ん' {
            let next = lookahead_ipa(&chars, index + 1);
            output.push_str(coda(next.as_deref()));
            index += 1;
            geminate = false;
            continue;
        }
        let slice: String = chars[index..].iter().collect();
        let Some((consumed, mora)) = mora_at(&slice) else {
            index += 1;
            geminate = false;
            continue;
        };
        index += consumed;
        let mut long = false;
        if index < chars.len() {
            let next = chars[index];
            if next == 'ー'
                || (mora.vowel == 'o' && next == 'う')
                || (mora.vowel == 'e' && next == 'い')
                || same_vowel(next, mora.vowel)
            {
                long = true;
                index += 1;
            }
        }
        let mut ipa = mora.ipa.to_owned();
        if geminate {
            ipa = geminate_ipa(&ipa);
            geminate = false;
        }
        output.push_str(&ipa);
        if long {
            push_length(&mut output);
        }
    }
    output
}

struct Mora {
    ipa: &'static str,
    vowel: char,
}

fn mora_at(text: &str) -> Option<(usize, Mora)> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if let Some(second) = chars.next() {
        let two: String = [first, second].into_iter().collect();
        if let Some(mora) = digraph(&two) {
            return Some((2, mora));
        }
    }
    monograph(first).map(|mora| (1, mora))
}

fn digraph(text: &str) -> Option<Mora> {
    let (ipa, vowel) = match text {
        "きゃ" => ("kja", 'a'),
        "きゅ" => ("kjɯ", 'u'),
        "きょ" => ("kjo", 'o'),
        "ぎゃ" => ("ɡja", 'a'),
        "ぎゅ" => ("ɡjɯ", 'u'),
        "ぎょ" => ("ɡjo", 'o'),
        "しゃ" => ("ɕa", 'a'),
        "しゅ" => ("ɕɯ", 'u'),
        "しょ" => ("ɕo", 'o'),
        "じゃ" => ("dʑa", 'a'),
        "じゅ" => ("dʑɯ", 'u'),
        "じょ" => ("dʑo", 'o'),
        "ちゃ" => ("tɕa", 'a'),
        "ちゅ" => ("tɕɯ", 'u'),
        "ちょ" => ("tɕo", 'o'),
        "にゃ" => ("ɲa", 'a'),
        "にゅ" => ("ɲɯ", 'u'),
        "にょ" => ("ɲo", 'o'),
        "ひゃ" => ("ça", 'a'),
        "ひゅ" => ("çɯ", 'u'),
        "ひょ" => ("ço", 'o'),
        "びゃ" => ("bja", 'a'),
        "びゅ" => ("bjɯ", 'u'),
        "びょ" => ("bjo", 'o'),
        "ぴゃ" => ("pja", 'a'),
        "ぴゅ" => ("pjɯ", 'u'),
        "ぴょ" => ("pjo", 'o'),
        "みゃ" => ("mja", 'a'),
        "みゅ" => ("mjɯ", 'u'),
        "みょ" => ("mjo", 'o'),
        "りゃ" => ("ɾja", 'a'),
        "りゅ" => ("ɾjɯ", 'u'),
        "りょ" => ("ɾjo", 'o'),
        "ふぁ" => ("ɸa", 'a'),
        "ふぃ" => ("ɸi", 'i'),
        "ふぇ" => ("ɸe", 'e'),
        "ふぉ" => ("ɸo", 'o'),
        "ゔぁ" => ("va", 'a'),
        "ゔぃ" => ("vi", 'i'),
        "ゔぇ" => ("ve", 'e'),
        "ゔぉ" => ("vo", 'o'),
        "うぃ" => ("wi", 'i'),
        "うぇ" => ("we", 'e'),
        "うぉ" => ("wo", 'o'),
        "てぃ" => ("ti", 'i'),
        "でぃ" => ("di", 'i'),
        "しぇ" => ("ɕe", 'e'),
        "じぇ" => ("dʑe", 'e'),
        "ちぇ" => ("tɕe", 'e'),
        "とぅ" => ("tɯ", 'u'),
        "どぅ" => ("dɯ", 'u'),
        "つぁ" => ("tsa", 'a'),
        "つぃ" => ("tsi", 'i'),
        "つぇ" => ("tse", 'e'),
        "つぉ" => ("tso", 'o'),
        "くぁ" => ("kwa", 'a'),
        "ぐぁ" => ("ɡwa", 'a'),
        _ => return None,
    };
    Some(Mora { ipa, vowel })
}

fn monograph(character: char) -> Option<Mora> {
    let (ipa, vowel) = match character {
        'あ' | 'ぁ' => ("a", 'a'),
        'い' | 'ぃ' => ("i", 'i'),
        'う' | 'ぅ' => ("ɯ", 'u'),
        'え' | 'ぇ' => ("e", 'e'),
        'お' | 'ぉ' => ("o", 'o'),
        'か' => ("ka", 'a'),
        'き' => ("ki", 'i'),
        'く' => ("kɯ", 'u'),
        'け' => ("ke", 'e'),
        'こ' => ("ko", 'o'),
        'さ' => ("sa", 'a'),
        'し' => ("ɕi", 'i'),
        'す' => ("sɯ", 'u'),
        'せ' => ("se", 'e'),
        'そ' => ("so", 'o'),
        'た' => ("ta", 'a'),
        'ち' => ("tɕi", 'i'),
        'つ' => ("tsɯ", 'u'),
        'て' => ("te", 'e'),
        'と' => ("to", 'o'),
        'な' => ("na", 'a'),
        'に' => ("ɲi", 'i'),
        'ぬ' => ("nɯ", 'u'),
        'ね' => ("ne", 'e'),
        'の' => ("no", 'o'),
        'は' => ("ha", 'a'),
        'ひ' => ("çi", 'i'),
        'ふ' => ("ɸɯ", 'u'),
        'へ' => ("he", 'e'),
        'ほ' => ("ho", 'o'),
        'ま' => ("ma", 'a'),
        'み' => ("mi", 'i'),
        'む' => ("mɯ", 'u'),
        'め' => ("me", 'e'),
        'も' => ("mo", 'o'),
        'や' | 'ゃ' => ("ja", 'a'),
        'ゆ' | 'ゅ' => ("jɯ", 'u'),
        'よ' | 'ょ' => ("jo", 'o'),
        'ら' => ("ɾa", 'a'),
        'り' => ("ɾi", 'i'),
        'る' => ("ɾɯ", 'u'),
        'れ' => ("ɾe", 'e'),
        'ろ' => ("ɾo", 'o'),
        'わ' | 'ゎ' => ("wa", 'a'),
        'を' => ("o", 'o'),
        'が' => ("ɡa", 'a'),
        'ぎ' => ("ɡi", 'i'),
        'ぐ' => ("ɡɯ", 'u'),
        'げ' => ("ɡe", 'e'),
        'ご' => ("ɡo", 'o'),
        'ざ' => ("za", 'a'),
        'じ' => ("dʑi", 'i'),
        'ず' => ("zɯ", 'u'),
        'ぜ' => ("ze", 'e'),
        'ぞ' => ("zo", 'o'),
        'だ' => ("da", 'a'),
        'ぢ' => ("dʑi", 'i'),
        'づ' => ("dzɯ", 'u'),
        'で' => ("de", 'e'),
        'ど' => ("do", 'o'),
        'ば' => ("ba", 'a'),
        'び' => ("bi", 'i'),
        'ぶ' => ("bɯ", 'u'),
        'べ' => ("be", 'e'),
        'ぼ' => ("bo", 'o'),
        'ぱ' => ("pa", 'a'),
        'ぴ' => ("pi", 'i'),
        'ぷ' => ("pɯ", 'u'),
        'ぺ' => ("pe", 'e'),
        'ぽ' => ("po", 'o'),
        'ゔ' => ("vɯ", 'u'),
        _ => return None,
    };
    Some(Mora { ipa, vowel })
}

fn same_vowel(character: char, vowel: char) -> bool {
    matches!(
        (character, vowel),
        ('あ' | 'ぁ', 'a')
            | ('い' | 'ぃ', 'i')
            | ('う' | 'ぅ', 'u')
            | ('え' | 'ぇ', 'e')
            | ('お' | 'ぉ', 'o')
    )
}

fn lookahead_ipa(chars: &[char], index: usize) -> Option<String> {
    if index >= chars.len() {
        return None;
    }
    if matches!(chars[index], 'っ' | 'ー' | 'ん') {
        return None;
    }
    let slice: String = chars[index..].iter().collect();
    mora_at(&slice).map(|(_, mora)| mora.ipa.to_owned())
}

fn coda(next: Option<&str>) -> &'static str {
    let Some(next) = next else {
        return "ɴ";
    };
    match next.chars().next().unwrap_or('ɴ') {
        'a' | 'i' | 'ɯ' | 'e' | 'o' => "ɴ",
        'b' | 'p' | 'm' => "m",
        'k' | 'g' => "ŋ",
        _ => "n",
    }
}

fn geminate_ipa(ipa: &str) -> String {
    let mut chars = ipa.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let rest: String = chars.collect();
    format!("{first}ː{rest}")
}

fn push_length(output: &mut String) {
    if !output.is_empty() && !output.ends_with('ː') {
        output.push('ː');
    }
}

fn to_hiragana(text: &str) -> String {
    text.chars()
        .map(|character| {
            let code = character as u32;
            if (0x30A1..=0x30F6).contains(&code) {
                char::from_u32(code - 0x60).unwrap_or(character)
            } else {
                character
            }
        })
        .collect()
}

fn is_hiragana(character: char) -> bool {
    ('ぁ'..='ん').contains(&character) || character == 'ゔ'
}

#[cfg(test)]
mod tests {
    use super::{broaden, ipa_for};

    #[test]
    fn readings_use_broad_tokyo_ipa() {
        assert_eq!(ipa_for("猫", "ねこ"), "neko");
        assert_eq!(ipa_for("学校", "がっこう"), "ɡakːoː");
        assert_eq!(ipa_for("新聞", "しんぶん"), "ɕimbɯɴ");
        assert_eq!(ipa_for("今日", "きょう"), "kjoː");
        assert_eq!(ipa_for("学生", "がくせい"), "ɡakɯseː");
        assert_eq!(ipa_for("大きい", "おおきい"), "oːkiː");
        assert_eq!(ipa_for("コーヒー", "コーヒー"), "koːçiː");
        assert_eq!(ipa_for("パーティー", "パーティー"), "paːtiː");
    }

    #[test]
    fn dictionary_text_becomes_broad_ipa_without_pitch_or_narrowing() {
        assert_eq!(broaden("[ne̞ko̞]"), "neko");
        assert_eq!(broaden("/gakkoː/"), "ɡakːoː");
        assert_eq!(broaden("neꜜko"), "neko");
        assert_eq!(broaden("t͡ɕi"), "tɕi");
        assert_eq!(broaden("ttɕi"), "tːɕi");
        assert_eq!(broaden("ɡakːoː"), "ɡakːoː");
    }

    #[test]
    fn particles_use_their_spoken_forms() {
        assert_eq!(ipa_for("は", "は"), "wa");
        assert_eq!(ipa_for("へ", "へ"), "e");
        assert_eq!(ipa_for("を", "を"), "o");
        assert_eq!(ipa_for("歯", "は"), "ha");
    }
}
