// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Jisho is the primary source for senses. English Wiktionary is the backup
//! and supplies example sentences and IPA. Missing IPA is generated from kana.

use crate::japanese::ipa::{ipa_for, kana_key};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

const USER_AGENT: &str = "cosmic-applet-finance-dashboard/0.1 (https://github.com/JoeyTong/cosmic-applet-finance-dashboard; educational COSMIC applet)";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sense {
    pub pos: String,
    pub gloss: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Example {
    pub japanese: String,
    pub english: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detail {
    pub senses: Vec<Sense>,
    pub examples: Vec<Example>,
    pub ipa: String,
    pub ipa_generated: bool,
    pub from_jisho: bool,
    pub from_wiktionary: bool,
}

#[derive(Clone, Debug)]
pub struct Lookup {
    pub detail: Detail,
    pub cache: bool,
}

pub async fn lookup(word: &str, reading: &str) -> Lookup {
    let client = client();
    let (jisho, wiki) = tokio::join!(
        fetch_jisho(client, word, reading),
        fetch_wiktionary(client, word, reading)
    );
    assemble(word, reading, jisho.ok(), wiki.ok())
}

pub fn assemble(
    word: &str,
    reading: &str,
    jisho: Option<Vec<Sense>>,
    wikitext: Option<String>,
) -> Lookup {
    let from_jisho = jisho.as_ref().is_some_and(|senses| !senses.is_empty());
    let parsed = wikitext
        .as_deref()
        .map(|text| parse_wikitext(text, reading));
    let from_wiktionary = parsed.is_some();
    let mut senses = jisho.filter(|items| !items.is_empty()).unwrap_or_default();
    let mut examples = Vec::new();
    let mut ipa = None;
    if let Some(parsed) = parsed {
        if senses.is_empty() {
            senses = parsed.senses;
        }
        examples = parsed.examples;
        ipa = parsed.ipa;
    }
    let ipa_generated = ipa.is_none();
    for sense in &mut senses {
        sense.gloss = crate::japanese::british::to_british(&sense.gloss);
    }
    for example in &mut examples {
        example.english = crate::japanese::british::to_british(&example.english);
    }
    Lookup {
        detail: Detail {
            senses,
            examples,
            ipa: ipa.unwrap_or_else(|| ipa_for(word, reading)),
            ipa_generated,
            from_jisho,
            from_wiktionary,
        },
        cache: from_jisho || from_wiktionary,
    }
}

struct ParsedWiki {
    ipa: Option<String>,
    examples: Vec<Example>,
    senses: Vec<Sense>,
}

pub fn parse_jisho(json: &str, word: &str, reading: &str) -> Result<Vec<Sense>, String> {
    let response: JishoResponse = serde_json::from_str(json).map_err(|error| error.to_string())?;
    Ok(select_jisho(&response.data, word, reading))
}

fn parse_wikitext(text: &str, reading: &str) -> ParsedWiki {
    ParsedWiki {
        ipa: extract_ipa(text, reading),
        examples: extract_examples(text, reading),
        senses: extract_senses(text),
    }
}

fn select_jisho(entries: &[JishoEntry], word: &str, reading: &str) -> Vec<Sense> {
    let reading_key = kana_key(reading);
    let entry = entries
        .iter()
        .find(|entry| {
            entry.japanese.iter().any(|form| {
                form_word(form) == word
                    && kana_key(&form.reading.clone().unwrap_or_default()) == reading_key
            })
        })
        .or_else(|| {
            entries
                .iter()
                .find(|entry| entry.japanese.iter().any(|form| form_word(form) == word))
        })
        .or_else(|| {
            entries.iter().find(|entry| {
                entry
                    .japanese
                    .iter()
                    .any(|form| kana_key(&form.reading.clone().unwrap_or_default()) == reading_key)
            })
        });
    entry.map(senses_of).unwrap_or_default()
}

fn form_word(form: &JishoForm) -> String {
    form.word
        .clone()
        .or_else(|| form.reading.clone())
        .unwrap_or_default()
}

fn senses_of(entry: &JishoEntry) -> Vec<Sense> {
    entry
        .senses
        .iter()
        .filter(|sense| {
            !sense
                .parts_of_speech
                .iter()
                .any(|part| part.to_ascii_lowercase().contains("wikipedia"))
        })
        .filter(|sense| !sense.english_definitions.is_empty())
        .take(4)
        .map(|sense| Sense {
            pos: if sense.parts_of_speech.is_empty() {
                "Definition".to_owned()
            } else {
                sense.parts_of_speech.join(", ")
            },
            gloss: sense
                .english_definitions
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join("; "),
        })
        .collect()
}

fn extract_ipa(text: &str, reading: &str) -> Option<String> {
    let key = kana_key(reading);
    let prons = template_at(text, "ja-pron");
    let window = prons
        .iter()
        .find(|(args, _start)| args.first().is_some_and(|kana| kana_key(kana) == key))
        .map(|(_args, start)| {
            let from = text.floor_char_boundary(start.saturating_sub(2_500));
            &text[from..*start]
        })
        .unwrap_or(text);
    first_ipa(window).or_else(|| first_ipa(text))
}

fn first_ipa(text: &str) -> Option<String> {
    for (args, _start) in template_at(text, "IPAchar") {
        if let Some(ipa) = args.first().and_then(|value| clean_ipa(value)) {
            return Some(ipa);
        }
    }
    for (args, _start) in template_at(text, "IPA") {
        let raw = if args.first().is_some_and(|value| value.trim() == "ja") {
            args.get(1)
        } else {
            args.first()
        }?;
        if let Some(ipa) = clean_ipa(raw) {
            return Some(ipa);
        }
    }
    None
}

fn clean_ipa(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('/').trim();
    let trimmed = trimmed.split([',', '|']).next()?.trim();
    if trimmed.is_empty() || trimmed.contains('{') || trimmed.contains('}') {
        None
    } else {
        let broadened = crate::japanese::ipa::broaden(trimmed);
        if broadened.is_empty() {
            None
        } else {
            Some(broadened)
        }
    }
}

fn extract_examples(text: &str, reading: &str) -> Vec<Example> {
    let key = kana_key(reading);
    let mut matched = Vec::new();
    let mut others = Vec::new();
    for (args, _start) in template_at(text, "ja-usex") {
        if args.len() < 3 {
            continue;
        }
        push_example(
            &mut matched,
            &mut others,
            &key,
            &args[0],
            Some(&args[1]),
            &args[2],
        );
    }
    for (args, _start) in template_at(text, "ux") {
        if args.first().is_none_or(|lang| lang.trim() != "ja") || args.len() < 2 {
            continue;
        }
        let english = args
            .iter()
            .find_map(|arg| arg.trim().strip_prefix("t=").map(str::trim))
            .unwrap_or_else(|| args.get(2).map(String::as_str).unwrap_or(""));
        push_example(&mut matched, &mut others, &key, &args[1], None, english);
    }
    matched.extend(others);
    matched.truncate(3);
    matched
}

fn push_example(
    matched: &mut Vec<Example>,
    others: &mut Vec<Example>,
    reading: &str,
    japanese: &str,
    kana: Option<&str>,
    english: &str,
) {
    let japanese = clean_wiki(japanese);
    let english = clean_wiki(english);
    if kana_key(&japanese).is_empty()
        || japanese.is_empty()
        || english.is_empty()
        || japanese.chars().count() > 80
        || english.chars().count() > 160
    {
        return;
    }
    let example = Example { japanese, english };
    let kana_matches = kana.is_some_and(|kana| kana_key(kana).contains(reading));
    if kana_matches {
        matched.push(example);
    } else {
        others.push(example);
    }
}

fn extract_senses(text: &str) -> Vec<Sense> {
    let mut senses = Vec::new();
    let mut pos = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('=') {
            pos = part_of_speech(trimmed);
            continue;
        }
        if !trimmed.starts_with('#')
            || trimmed.starts_with("#:")
            || trimmed.starts_with("#*")
            || trimmed.starts_with("##")
        {
            continue;
        }
        if senses.len() >= 4 {
            break;
        }
        let gloss = clean_wiki(trimmed.trim_start_matches('#').trim());
        if gloss.is_empty() || gloss.contains("{{") {
            continue;
        }
        senses.push(Sense {
            pos: pos.clone().unwrap_or_else(|| "Definition".to_owned()),
            gloss,
        });
    }
    senses
}

fn part_of_speech(line: &str) -> Option<String> {
    let name = line.trim_matches('=').trim();
    const PARTS: &[&str] = &[
        "Noun",
        "Verb",
        "Adjective",
        "Adverb",
        "Pronoun",
        "Particle",
        "Interjection",
        "Counter",
        "Suffix",
        "Prefix",
        "Proper noun",
        "Phrase",
        "Conjunction",
        "Determiner",
        "Numeral",
    ];
    PARTS
        .iter()
        .any(|part| name.starts_with(part))
        .then(|| name.to_owned())
}

pub fn clean_wiki(input: &str) -> String {
    let without_tags = strip_tags(input);
    let chars: Vec<char> = without_tags.chars().collect();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '\'' && index + 1 < chars.len() && chars[index + 1] == '\'' {
            index += 2;
            if index < chars.len() && chars[index] == '\'' {
                index += 1;
            }
            continue;
        }
        if chars[index] == '[' && index + 1 < chars.len() && chars[index + 1] == '[' {
            index += 2;
            let start = index;
            while index + 1 < chars.len() && !(chars[index] == ']' && chars[index + 1] == ']') {
                index += 1;
            }
            let inner: String = chars[start..index.min(chars.len())].iter().collect();
            if index + 1 < chars.len() {
                index += 2;
            }
            output.push_str(&link_label(&inner));
            continue;
        }
        output.push(chars[index]);
        index += 1;
    }
    decode_entities(&output)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn link_label(inner: &str) -> String {
    let label = inner
        .rsplit_once('|')
        .map(|(_, label)| label)
        .unwrap_or_else(|| inner.split('#').next().unwrap_or(inner));
    label.replace('%', "")
}

fn strip_tags(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '<' {
            for next in chars.by_ref() {
                if next == '>' {
                    break;
                }
            }
        } else {
            output.push(character);
        }
    }
    output
}

fn decode_entities(input: &str) -> String {
    input
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn template_at(text: &str, name: &str) -> Vec<(Vec<String>, usize)> {
    let needle = format!("{{{{{name}");
    let mut found = Vec::new();
    let mut start = 0;
    while let Some(relative) = text[start..].find(&needle) {
        let at = start + relative;
        let after = at + needle.len();
        let boundary = text.as_bytes().get(after).copied();
        if matches!(boundary, Some(b'|' | b'}'))
            && let Some(args) = parse_template(text, at)
        {
            found.push((args, at));
        }
        start = after.max(start + 1);
    }
    found
}

fn parse_template(text: &str, start: usize) -> Option<Vec<String>> {
    let rest = text.get(start..)?;
    if !rest.starts_with("{{") {
        return None;
    }
    let mut depth = 0;
    let mut link_depth: usize = 0;
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut chars = rest.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '{' && chars.peek() == Some(&'{') {
            chars.next();
            depth += 1;
            if depth > 1 {
                current.push_str("{{");
            }
            continue;
        }
        if character == '}' && chars.peek() == Some(&'}') {
            chars.next();
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                pieces.push(std::mem::take(&mut current));
                if pieces.is_empty() {
                    return Some(Vec::new());
                }
                pieces.remove(0);
                return Some(
                    pieces
                        .into_iter()
                        .map(|piece| piece.trim().to_owned())
                        .collect(),
                );
            }
            current.push_str("}}");
            continue;
        }
        if depth == 1 && link_depth == 0 && character == '|' {
            pieces.push(std::mem::take(&mut current));
            continue;
        }
        if character == '[' && chars.peek() == Some(&'[') {
            chars.next();
            link_depth += 1;
            current.push_str("[[");
            continue;
        }
        if character == ']' && chars.peek() == Some(&']') {
            chars.next();
            link_depth = link_depth.saturating_sub(1);
            current.push_str("]]");
            continue;
        }
        if depth >= 1 {
            current.push(character);
        }
    }
    None
}

async fn fetch_jisho(
    client: &reqwest::Client,
    word: &str,
    reading: &str,
) -> Result<Vec<Sense>, String> {
    let mut senses = jisho_keyword(client, word, word, reading).await?;
    if senses.is_empty() && kana_key(word) != kana_key(reading) {
        senses = jisho_keyword(client, reading, word, reading).await?;
    }
    Ok(senses)
}

async fn jisho_keyword(
    client: &reqwest::Client,
    keyword: &str,
    word: &str,
    reading: &str,
) -> Result<Vec<Sense>, String> {
    let url = format!(
        "https://jisho.org/api/v1/search/words?keyword={}",
        encode(keyword)
    );
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Jisho returned {}", response.status()));
    }
    let body = response.text().await.map_err(|error| error.to_string())?;
    parse_jisho(&body, word, reading)
}

async fn fetch_wiktionary(
    client: &reqwest::Client,
    word: &str,
    reading: &str,
) -> Result<String, String> {
    match wikitext(client, word).await {
        Ok(text) => Ok(text),
        Err(error) if kana_key(word) != kana_key(reading) => {
            wikitext(client, reading).await.map_err(|_| error)
        }
        Err(error) => Err(error),
    }
}

async fn wikitext(client: &reqwest::Client, title: &str) -> Result<String, String> {
    let sections_url = format!(
        "https://en.wiktionary.org/w/api.php?action=parse&page={}&prop=sections&format=json&formatversion=2&redirects=1",
        encode(title)
    );
    let sections: serde_json::Value = send_json(client, &sections_url).await?;
    let index = japanese_index(&sections)?;
    let text_url = format!(
        "https://en.wiktionary.org/w/api.php?action=parse&page={}&prop=wikitext&section={}&format=json&formatversion=2&redirects=1",
        encode(title),
        encode(&index)
    );
    let parsed: serde_json::Value = send_json(client, &text_url).await?;
    parsed
        .pointer("/parse/wikitext")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "Wiktionary did not return wikitext".to_owned())
}

async fn send_json(client: &reqwest::Client, url: &str) -> Result<serde_json::Value, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Wiktionary returned {}", response.status()));
    }
    response.json().await.map_err(|error| error.to_string())
}

fn japanese_index(value: &serde_json::Value) -> Result<String, String> {
    if let Some(error) = value.get("error") {
        return Err(error.to_string());
    }
    let sections = value
        .pointer("/parse/sections")
        .and_then(|item| item.as_array())
        .ok_or("Wiktionary did not return sections")?;
    for section in sections {
        let line = section
            .get("line")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        let level = section.get("level").and_then(|item| {
            item.as_i64()
                .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
        });
        if line == "Japanese" && level == Some(2) {
            if let Some(index) = section.get("index").and_then(|item| item.as_str()) {
                return Ok(index.to_owned());
            }
            if let Some(index) = section.get("index").and_then(|item| item.as_i64()) {
                return Ok(index.to_string());
            }
        }
    }
    Err("Wiktionary has no Japanese section".to_owned())
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(12))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn encode(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[derive(Deserialize)]
struct JishoResponse {
    data: Vec<JishoEntry>,
}

#[derive(Deserialize)]
struct JishoEntry {
    japanese: Vec<JishoForm>,
    senses: Vec<JishoSense>,
}

#[derive(Deserialize)]
struct JishoForm {
    #[serde(default)]
    word: Option<String>,
    #[serde(default)]
    reading: Option<String>,
}

#[derive(Deserialize)]
struct JishoSense {
    #[serde(default)]
    english_definitions: Vec<String>,
    #[serde(default)]
    parts_of_speech: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const JISHO: &str = r#"{
      "data": [
        {
          "japanese": [{"word": "猫", "reading": "ねこ"}],
          "senses": [
            {"english_definitions": ["cat", "feline"], "parts_of_speech": ["Noun"]},
            {"english_definitions": ["Cat"], "parts_of_speech": ["Wikipedia definition"]}
          ]
        }
      ]
    }"#;

    const WIKI: &str = r#"
{{IPAchar|/neko/}}
{{ja-pron|ねこ|acc=1}}
===Noun===
# [[cat]]
#: {{ja-usex|'''猫'''[[が#Etymology 2|が]][[居る]][[。]]|'''ねこ''' が いる。|There is a '''cat'''.}}
#: {{ja-usex|家には猫がいます。|いえ に は ねこ が います。|There is a cat in the house.}}
: {{ux|ja|家狸一名猫|t=A classical citation without kana.}}
: {{ux|ja|猫: この引用は学習カードに入れるには長すぎる文章であり、八十文字を超えるので除外される。さらに説明を足して、カードの例文としては残さない長さにする。追加の文言です。|t=This citation is far too long to keep on a study card and should be dropped.}}
"#;

    #[test]
    fn jisho_senses_skip_wikipedia_and_keep_the_part_of_speech() {
        let senses = parse_jisho(JISHO, "猫", "ねこ").unwrap();
        assert_eq!(senses.len(), 1);
        assert_eq!(senses[0].pos, "Noun");
        assert_eq!(senses[0].gloss, "cat; feline");
    }

    #[test]
    fn wiktionary_provides_ipa_and_up_to_three_short_examples() {
        let parsed = parse_wikitext(WIKI, "ねこ");
        assert_eq!(parsed.ipa.as_deref(), Some("neko"));
        assert_eq!(parsed.examples.len(), 2);
        assert_eq!(parsed.examples[0].japanese, "猫が居る。");
        assert_eq!(parsed.examples[0].english, "There is a cat.");
        assert_eq!(parsed.senses[0].gloss, "cat");
        assert_eq!(parsed.senses[0].pos, "Noun");
    }

    #[test]
    fn ipa_lookup_does_not_slice_through_a_multibyte_character() {
        let text = format!(
            "{}{{{{IPAchar|/neko/}}}}{{{{ja-pron|ねこ}}}}",
            "あ".repeat(900)
        );
        let parsed = parse_wikitext(&text, "ねこ");
        assert_eq!(parsed.ipa.as_deref(), Some("neko"));
    }

    #[test]
    fn jisho_senses_win_and_wiktionary_still_supplies_examples() {
        let lookup = assemble(
            "猫",
            "ねこ",
            parse_jisho(JISHO, "猫", "ねこ").ok(),
            Some(WIKI.to_owned()),
        );
        assert!(lookup.cache);
        assert!(lookup.detail.from_jisho);
        assert!(lookup.detail.from_wiktionary);
        assert!(!lookup.detail.ipa_generated);
        assert_eq!(lookup.detail.ipa, "neko");
        assert_eq!(lookup.detail.senses[0].gloss, "cat; feline");
        assert_eq!(lookup.detail.examples.len(), 2);
    }

    #[test]
    fn both_sources_missing_still_generates_ipa_without_caching() {
        let lookup = assemble("猫", "ねこ", None, None);
        assert!(!lookup.cache);
        assert!(lookup.detail.ipa_generated);
        assert_eq!(lookup.detail.ipa, "neko");
        assert!(lookup.detail.examples.is_empty());
    }
}
