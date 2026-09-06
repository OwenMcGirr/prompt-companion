use serde::{Deserialize, Serialize};
use serde_json::Value;
use unicode_normalization::char::is_combining_mark;
use unicode_segmentation::UnicodeSegmentation;

pub fn count(s: &str) -> usize {
    s.graphemes(true).count()
}
pub fn prefix(s: &str, n: usize) -> String {
    s.graphemes(true).take(n).collect()
}
pub fn suffix(s: &str, n: usize) -> String {
    let len = count(s);
    s.graphemes(true).skip(len.saturating_sub(n)).collect()
}
pub fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}
pub fn byte_offset(s: &str, offset: usize) -> usize {
    let mut units = 0;
    for (b, c) in s.char_indices() {
        if units + c.len_utf16() > offset {
            return b;
        }
        units += c.len_utf16();
    }
    s.len()
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Draft {
    pub text: String,
    pub cursor: usize,
    pub selection_length: usize,
}
impl Draft {
    pub fn new(text: impl Into<String>, cursor: usize, selection_length: usize) -> Self {
        let text = text.into();
        let start = byte_offset(&text, cursor);
        let end = byte_offset(&text, cursor.saturating_add(selection_length));
        Self {
            cursor: utf16(&text[..start]),
            selection_length: utf16(&text[start..end]),
            text,
        }
    }
    pub fn at_end(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::new(text.clone(), utf16(&text), 0)
    }
    pub fn normalized(&self) -> Self {
        Self::new(self.text.clone(), self.cursor, self.selection_length)
    }
}
fn word(c: char) -> bool {
    c.is_alphanumeric() || is_combining_mark(c) || ['_', '\'', '’'].contains(&c)
}
#[derive(Debug, Clone)]
pub struct Target {
    pub draft: Draft,
    pub before: String,
    pub partial: String,
    pub selected: String,
    pub after: String,
}
impl Target {
    pub fn new(draft: &Draft) -> Self {
        let draft = draft.normalized();
        let s = &draft.text;
        let cursor = byte_offset(s, draft.cursor);
        let mut start = cursor;
        let mut end = byte_offset(s, draft.cursor + draft.selection_length);
        if draft.selection_length == 0 {
            for (b, c) in s[..cursor].char_indices().rev() {
                if !word(c) {
                    break;
                }
                start = b;
            }
            if start < cursor {
                for (b, c) in s[cursor..].char_indices() {
                    if !word(c) {
                        break;
                    }
                    end = cursor + b + c.len_utf8();
                }
            }
        }
        Self {
            before: s[..start].into(),
            partial: if draft.selection_length == 0 {
                s[start..cursor].into()
            } else {
                String::new()
            },
            selected: s[start..end].into(),
            after: s[end..].into(),
            draft,
        }
    }
    pub fn normalize(&self, candidate: &str) -> Option<String> {
        let mut value = candidate.trim().to_string();
        if value.is_empty() || count(&value) > 160 || value.contains(['\n', '\r']) {
            return None;
        }
        if !self.partial.is_empty() {
            if !value
                .to_lowercase()
                .starts_with(&self.partial.to_lowercase())
            {
                return None;
            }
            value = format!(
                "{}{}",
                self.partial,
                value
                    .graphemes(true)
                    .skip(count(&self.partial))
                    .collect::<String>()
            );
        }
        (value != self.partial).then_some(value)
    }
    pub fn insert(&self, candidate: &str) -> Option<Draft> {
        let mut value = self.normalize(candidate)?;
        if value.chars().next().is_some_and(char::is_alphabetic)
            && self.before.ends_with([',', ';', ':', '!', '?'])
        {
            value.insert(0, ' ');
        }
        if self.after.is_empty() || self.after.chars().next().is_some_and(char::is_alphanumeric) {
            value.push(' ');
        }
        Some(Draft::new(
            format!("{}{}{}", self.before, value, self.after),
            utf16(&self.before) + utf16(&value),
            0,
        ))
    }
    pub fn phrases(&self, raw: &Value) -> Result<Vec<String>, String> {
        let values = raw["suggestions"]
            .as_array()
            .ok_or("Invalid phrase response")?;
        let mut phrases = Vec::new();
        for v in values {
            if let Some(s) = v.as_str().and_then(|s| self.normalize(s)) {
                if !phrases
                    .iter()
                    .any(|p: &String| p.to_lowercase() == s.to_lowercase())
                {
                    phrases.push(s)
                }
            }
        }
        phrases.truncate(3);
        if phrases.is_empty() {
            Err("No usable phrases arrived. Try Refresh phrases.".into())
        } else {
            Ok(phrases)
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub role: String,
    pub text: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Context {
    pub messages: Vec<Message>,
    pub partial: bool,
    pub active: bool,
}
pub fn messages(turns: &[Value]) -> Vec<Message> {
    let mut out = Vec::new();
    for turn in turns {
        for item in turn["items"].as_array().into_iter().flatten() {
            let (role, text) = match item["type"].as_str() {
                Some("userMessage") => (
                    "user",
                    item["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|c| c["type"] == "text")
                        .filter_map(|c| c["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                Some("agentMessage") => ("assistant", item["text"].as_str().unwrap_or("").into()),
                _ => continue,
            };
            if !text.is_empty() {
                out.push(Message {
                    role: role.into(),
                    text,
                })
            }
        }
    }
    out
}
pub fn bounded(items: &[Message], mut budget: usize) -> Vec<Message> {
    let mut out = Vec::new();
    for m in items.iter().rev() {
        if budget == 0 {
            break;
        }
        let text = prefix(
            &m.text,
            budget.min(if m.role == "user" { 3000 } else { 4500 }),
        );
        budget -= count(&text);
        out.push(Message {
            role: m.role.clone(),
            text,
        });
    }
    out.reverse();
    out
}
impl Context {
    pub fn recent(&self) -> Vec<Message> {
        bounded(
            &self.messages[self.messages.len().saturating_sub(12)..],
            12000,
        )
    }
    pub fn earlier(&self) -> Vec<Message> {
        let older = &self.messages[..self.messages.len().saturating_sub(12)];
        let opening = older.len().min(4);
        let mut out: Vec<_> = older[..opening]
            .iter()
            .map(|m| Message {
                role: m.role.clone(),
                text: prefix(&m.text, 750),
            })
            .collect();
        out.extend(bounded(&older[opening..], 5000));
        out
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Clarification {
    pub question: String,
    pub choices: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub question: String,
    pub choice: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expansion {
    Expanded(String),
    Clarification(Clarification),
}
pub fn expansion(raw: &Value, clarified: bool) -> Result<Expansion, String> {
    let err = "Couldn’t make a usable expansion. Your original words are safe.";
    let prompt = raw["prompt"].as_str().ok_or(err)?.trim();
    let question = raw["question"].as_str().ok_or(err)?.trim();
    let choices: Vec<String> = raw["choices"]
        .as_array()
        .ok_or(err)?
        .iter()
        .map(|v| v.as_str().map(|s| s.trim().to_string()).ok_or(err))
        .collect::<Result<_, _>>()?;
    match raw["kind"].as_str() {
        Some("expanded")
            if !prompt.is_empty()
                && count(prompt) <= 2000
                && prompt.split_whitespace().count() <= 180
                && question.is_empty()
                && choices.is_empty() =>
        {
            Ok(Expansion::Expanded(prompt.into()))
        }
        Some("clarification")
            if !clarified
                && prompt.is_empty()
                && !question.is_empty()
                && count(question) <= 140
                && (2..=3).contains(&choices.len())
                && choices
                    .iter()
                    .all(|s| !s.is_empty() && count(s) <= 70 && !s.contains(['\n', '\r']))
                && choices
                    .iter()
                    .map(|s| s.to_lowercase())
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == choices.len() =>
        {
            Ok(Expansion::Clarification(Clarification {
                question: question.into(),
                choices,
            }))
        }
        _ => Err(err.into()),
    }
}
