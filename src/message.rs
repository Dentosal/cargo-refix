//! https://doc.rust-lang.org/rustc/json.html

#![allow(dead_code)]

use colored::Colorize;
use std::{collections::HashMap, fmt::Display, ops};

use crate::text::underline_span;

#[derive(Debug, serde::Deserialize)]
pub struct Msg {
    pub reason: String,
    #[serde(default)]
    pub package_id: String,
    #[serde(default)]
    pub manifest_path: String,
    pub target: Option<Target>,
    pub message: Option<CompilerMessage>,

    #[serde(flatten)]
    other: HashMap<String, serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CompilerMessage {
    pub code: Option<CompilerMessageCode>,
    pub level: String,
    pub message: String,
    pub spans: Vec<Span>,

    pub children: Vec<CompilerMessage>,

    #[serde(flatten)]
    other: HashMap<String, serde_json::Value>,
}

impl CompilerMessage {
    /// This is fixable by itself
    pub fn is_singular(&self) -> bool {
        self.level != "failure-note" && !self.message.starts_with("aborting due")
    }

    /// Error code or lint name as text, if any
    pub fn code(&self) -> Option<&str> {
        self.code
            .as_ref()
            .map(|code| code.code.as_ref())
            .flatten()
            .map(|code| code.as_str())
    }

    pub fn primary_spans(&self) -> impl Iterator<Item = &Span> + '_ {
        self.spans.iter().filter(|s| s.is_primary)
    }

    /// Help items containsing suggestions
    pub fn help_items(&self) -> impl Iterator<Item = &Span> + '_ {
        self.children
            .iter()
            .filter(|child| child.level == "help")
            .flat_map(|child| {
                child
                    .spans
                    .iter()
                    .filter(|span| span.suggested_replacement.is_some())
            })
    }

    pub fn spans_with_suggestions(&self) -> impl Iterator<Item = SpanAndSuggestions> + '_ {
        self.primary_spans().map(|primary| {
            let mut suggestions: Vec<_> = self
                .help_items()
                .filter(|help| primary.raw_text() == help.raw_text() && help.text.len() == 1)
                .map(|s| {
                    let replacement = s.suggested_replacement.as_ref().unwrap();
                    let applicability = s
                        .suggestion_applicability
                        .unwrap_or(SuggestionApplicability::Unspecified);
                    (
                        s.text[0].highlighted_span(),
                        replacement.clone(),
                        applicability,
                    )
                })
                .collect();

            suggestions.sort_by_key(|(r, _, _)| r.start);

            SpanAndSuggestions {
                primary: primary.clone(),
                suggestions,
            }
        })
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct CompilerMessageCode {
    pub code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SpanAndSuggestions {
    pub primary: Span,
    pub suggestions: Vec<(ops::Range<usize>, String, SuggestionApplicability)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
pub enum SuggestionApplicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Span {
    pub file_name: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub column_start: usize,
    pub column_end: usize,

    pub text: Vec<SpanText>,
    pub label: Option<String>,
    pub is_primary: bool,

    pub suggested_replacement: Option<String>,
    pub suggestion_applicability: Option<SuggestionApplicability>,

    #[serde(flatten)]
    other: HashMap<String, serde_json::Value>,
}

impl Span {
    pub fn raw_text(&self) -> String {
        self.text.iter().map(|text| text.text.clone()).collect()
    }
}

impl Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for text in &self.text {
            write!(f, "{}", text)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SpanText {
    pub highlight_end: usize,
    pub highlight_start: usize,
    pub text: String,
}
impl SpanText {
    pub fn highlighted_span(&self) -> ops::Range<usize> {
        (self.highlight_start - 1)..(self.highlight_end - 1)
    }

    pub fn highlighted(&self) -> &str {
        &self.text[(self.highlight_start - 1)..(self.highlight_end - 1)]
    }

    pub fn replace_highlighted(&self, replacement: &str) -> String {
        format!(
            "{}{}{}",
            &self.text[..(self.highlight_start - 1)],
            replacement,
            &self.text[(self.highlight_end - 1)..]
        )
    }
}

impl Display for SpanText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", underline_span(&self.text, self.highlighted_span()))
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct Target {
    pub kind: Vec<String>,
    pub name: String,
    pub src_path: String,
    #[serde(default)]
    features: Vec<String>,
}
