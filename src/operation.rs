use std::{collections::VecDeque, ops, path::PathBuf, str::FromStr};

use clap::Args;
use colored::Colorize;
use regex::Regex;
use similar::{ChangeTag, TextDiff};
use strum::EnumProperty;

use crate::{
    apply::{Change, Patch},
    message::{self, SpanAndSuggestions},
    selector,
    text::{find_matching_paren, template},
};

#[derive(Debug, Clone, Copy, strum::EnumString, strum::EnumProperty)]
pub enum TextOperation {
    /// Drop topmost stack element
    #[strum(serialize = "stack-drop", serialize = "s-drop")]
    #[strum(props(argc = "0"))]
    StackDrop,
    /// Duplicate topmost stack element
    #[strum(serialize = "stack-dup", serialize = "s-dup")]
    #[strum(props(argc = "0"))]
    StackDup,
    /// Push currently selected text to the stack
    #[strum(serialize = "stack-push", serialize = "s-push", serialize = "push")]
    #[strum(props(argc = "0"))]
    StackPush,
    /// Apply regex to the entire text instead of just the highlighted span
    #[strum(serialize = "whole")]
    #[strum(props(argc = "0"))]
    Whole,
    /// Original span highlighted in the compiler message
    #[strum(serialize = "original")]
    #[strum(props(argc = "0"))]
    Original,
    /// Require that only a single paren char is selected, and select the matching one
    #[strum(serialize = "matching-paren", serialize = "mp")]
    #[strum(props(argc = "0"))]
    MatchingParen,
    /// Select space between matching parens, including the parens themselves.
    /// If a single paren is selected, selects the are between it and the matching paren.
    /// If a span of parens is already selected, do nothing.
    /// Otherwise, expand the selection backwards until a paren is found and then select.
    #[strum(serialize = "parens")]
    #[strum(props(argc = "0"))]
    Parens,
    /// Extend selection forwards
    #[strum(serialize = "extend", serialize = "e")]
    #[strum(props(argc = "1"))]
    Extend,
    /// Find first match in the whole span
    #[strum(serialize = "first", serialize = "f")]
    #[strum(props(argc = "1"))]
    First,
    /// Match backwards from the selection, using match of this regex instead
    #[strum(serialize = "previous", serialize = "prev", serialize = "p")]
    #[strum(props(argc = "1"))]
    Previous,
    /// Match forwards from the selection, using match of this regex instead
    #[strum(serialize = "next", serialize = "n")]
    #[strum(props(argc = "1"))]
    Next,
    /// Limit the current selection to zero width, keeping the same start point
    #[strum(serialize = "zero")]
    #[strum(props(argc = "0"))]
    Zero,
    /// Select first match inside the current selection
    #[strum(serialize = "narrow", serialize = "inner")]
    #[strum(props(argc = "1"))]
    Narrow,
    /// Delete the current selection
    #[strum(serialize = "delete", serialize = "d")]
    #[strum(props(argc = "0"))]
    Delete,
    /// Replace the current selection with a string
    #[strum(serialize = "replace")]
    #[strum(props(argc = "1"))]
    Replace,
    /// Substitute the first regex match in the current selection
    #[strum(serialize = "substitute", serialize = "sub", serialize = "s")]
    #[strum(props(argc = "2"))]
    Substitute,
    /// Substitute all regex matches in the current selection
    #[strum(
        serialize = "substitute-all",
        serialize = "sub-all",
        serialize = "suba",
        serialize = "sa"
    )]
    #[strum(props(argc = "2"))]
    SubstituteAll,
}

impl TextOperation {
    pub fn apply(
        &self,
        stack: &mut Vec<String>,
        haystack: &mut String,
        original_span: ops::Range<usize>,
        span: ops::Range<usize>,
        args: &[&str],
    ) -> Result<ops::Range<usize>, ExecError> {
        macro_rules! regex_arg {
            ($index:literal) => {
                Regex::new(args[$index])
                    .map_err(|err| ExecError::InvalidRegex(args[$index].to_owned(), err))?
            };
        }

        let template_resolver = |name: &str| -> Result<Option<String>, ExecError> {
            match name {
                "top" => {
                    let value = stack.last().ok_or(ExecError::StackUnderflow(*self))?;
                    Ok(Some(value.clone()))
                }
                "pop" => {
                    let value = stack.pop().ok_or(ExecError::StackUnderflow(*self))?;
                    Ok(Some(value))
                }
                _ => Ok(None),
            }
        };

        macro_rules! string_arg {
            ($index:literal) => {{
                let value = args[$index];
                template(value, template_resolver)?
            }};
        }

        match self {
            TextOperation::StackDrop => {
                if stack.pop().is_none() {
                    return Err(ExecError::StackUnderflow(*self));
                }
                Ok(span)
            }
            TextOperation::StackDup => {
                let Some(v) = stack.pop() else {
                    return Err(ExecError::StackUnderflow(*self));
                };
                stack.push(v.clone());
                stack.push(v);
                Ok(span)
            }
            TextOperation::StackPush => {
                stack.push(haystack[span.clone()].to_owned());
                Ok(span)
            }
            TextOperation::Whole => Ok(0..haystack.len()),
            TextOperation::Original => Ok(original_span),
            TextOperation::MatchingParen => {
                if span.len() != 1 {
                    return Err(ExecError::NoMatches(*self));
                }

                let mp =
                    find_matching_paren(haystack, span.start).ok_or(ExecError::NoMatches(*self))?;
                Ok(mp..mp + 1)
            }
            TextOperation::Parens => {
                let mut span = span;
                loop {
                    if let Some(mp) = find_matching_paren(haystack, span.start) {
                        return Ok(if mp < span.start {
                            mp..span.start + 1
                        } else {
                            span.start..mp + 1
                        });
                    }

                    if span.start == 0 {
                        return Err(ExecError::NoMatches(*self));
                    }

                    loop {
                        span.start -= 1;
                        if haystack.is_char_boundary(span.start) {
                            break;
                        }
                    }
                }
            }
            TextOperation::Extend => {
                let m = regex_arg!(0)
                    .find_at(haystack, span.end)
                    .ok_or(ExecError::NoMatches(*self))?;
                if m.start() == span.end {
                    Ok(span.start..m.end())
                } else {
                    Ok(span)
                }
            }
            TextOperation::First => Ok(regex_arg!(0)
                .find(haystack)
                .ok_or(ExecError::NoMatches(*self))?
                .range()),
            TextOperation::Previous => Ok(regex_arg!(0)
                .find_iter(&haystack[..span.start])
                .last()
                .ok_or(ExecError::NoMatches(*self))?
                .range()),
            TextOperation::Next => Ok(regex_arg!(0)
                .find_at(haystack, span.end)
                .ok_or(ExecError::NoMatches(*self))?
                .range()),
            TextOperation::Narrow => Ok(regex_arg!(0)
                .find_at(&haystack[..span.end], span.start)
                .ok_or(ExecError::NoMatches(*self))?
                .range()),
            TextOperation::Zero => Ok(span.start..span.start),
            TextOperation::Delete => {
                haystack.replace_range(span.clone(), "");
                Ok(span.start..span.start)
            }
            TextOperation::Replace => {
                let value = string_arg!(0);
                haystack.replace_range(span.clone(), &value);
                Ok(span.start..span.start + value.len())
            }
            TextOperation::Substitute => {
                let replaced = regex_arg!(0)
                    .replace(&haystack[span.clone()], string_arg!(1))
                    .into_owned();
                haystack.replace_range(span.clone(), &replaced);
                Ok(span.start..span.start + replaced.len())
            }
            TextOperation::SubstituteAll => {
                let replaced = regex_arg!(0)
                    .replace_all(&haystack[span.clone()], string_arg!(1))
                    .into_owned();
                haystack.replace_range(span.clone(), &replaced);
                Ok(span.start..span.start + replaced.len())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExecError {
    /// No such operation
    UnknownOp(String),
    /// Argument could not be parsed as a regex
    InvalidRegex(String, regex::Error),
    /// No regex or other matches by operation
    NoMatches(TextOperation),
    /// Not enough arguments available
    NotEnoughArguments(TextOperation, usize),
    /// Cannot pop from empty stack
    StackUnderflow(TextOperation),
}
impl ExecError {
    /// Do not attempt to continue to next item after this
    pub fn stop_all(&self) -> bool {
        !matches!(self, Self::NoMatches(_))
    }
}

#[derive(Debug, Clone, Args)]
pub struct Operation {
    /// Apply suggestion provided by rustc first
    #[arg(short = 'a', long = "auto", alias = "suggestion")]
    suggestion: bool,

    /// Sequence of operations to apply
    ops: Vec<String>,
}

impl Operation {
    /// Run the operation sequence, mutating the given string
    pub fn run(&self, haystack: &mut String, mut span: ops::Range<usize>) -> Result<(), ExecError> {
        let orginal_span = span.clone();
        let mut ops: VecDeque<_> = self.ops.iter().collect();
        let mut stack = Vec::new();

        while let Some(op) = ops.pop_front() {
            let op =
                TextOperation::from_str(op).map_err(|_| ExecError::UnknownOp(op.to_owned()))?;
            let argc = op.get_str("argc").expect("missing argc property");
            let argc: usize = argc.parse().expect("invalid argc property");
            let mut args = Vec::with_capacity(argc);
            for _ in 0..argc {
                args.push(
                    ops.pop_front()
                        .ok_or(ExecError::NotEnoughArguments(op, args.len()))?
                        .as_str(),
                );
            }

            span = op.apply(&mut stack, haystack, orginal_span.clone(), span, &args)?;
        }

        Ok(())
    }

    pub fn compute_diffs(&self, target: &message::CompilerMessage) -> Result<Vec<Change>, ()> {
        let mut changes = Vec::new();
        'spans: for SpanAndSuggestions {
            primary: span,
            suggestions,
        } in target.spans_with_suggestions()
        {
            let mut new = String::new();
            for part in span.text.iter() {
                let mut selection = part.highlighted_span();

                let mut new_text = part.text.clone();

                if self.suggestion {
                    for (s_range, s_text, _) in suggestions.clone().into_iter().rev() {
                        if s_range.end <= selection.start {
                            selection.start -= s_text.len();
                            selection.end -= s_text.len();
                        } else if s_range.end <= selection.end {
                            let overlap = selection.end - s_range.end;
                            selection.start = s_range.start;
                            selection.end = selection.start + overlap;
                        } else if s_range.start <= selection.end {
                            selection.end = s_range.start;
                            selection.start = selection.start.min(selection.end);
                        }

                        new_text.replace_range(s_range, &s_text);
                    }
                }

                if let Err(err) = self.run(&mut new_text, selection.clone()) {
                    println!("{}:{}:", span.file_name, span.line_start);
                    println!(" Execution failed: {:?}", err);
                    if err.stop_all() {
                        return Err(());
                    } else {
                        continue 'spans;
                    }
                }
                new.push_str(&new_text);
            }

            changes.push(Change {
                file: PathBuf::from(&span.file_name),
                patch: Patch {
                    location: span.outer_byte_range(),
                    bytes: new.bytes().collect(),
                },
            });
        }
        Ok(changes)
    }

    pub fn preview(&self, target: &message::CompilerMessage, changes: &[Change]) {
        for (span, change) in target.spans.iter().zip(changes) {
            print!("{}:{}:", span.file_name, span.line_start);
            if let Some(label) = span.label.as_ref() {
                print!(" {}", label);
            }
            println!();
            show_text_diff(
                &span.raw_text(),
                &String::from_utf8_lossy(&change.patch.bytes),
            );
        }
    }
}

fn show_text_diff(old: &str, new: &str) {
    let diff = TextDiff::from_graphemes(old, new);

    let before: String = diff
        .iter_all_changes()
        .filter_map(|c| match c.tag() {
            ChangeTag::Equal => Some(c.value().to_string()),
            ChangeTag::Delete => Some(c.value().white().on_red().to_string()),
            ChangeTag::Insert => None,
        })
        .collect();

    let after: String = diff
        .iter_all_changes()
        .filter_map(|c| match c.tag() {
            ChangeTag::Equal => Some(c.value().to_string()),
            ChangeTag::Insert => Some(c.value().black().on_green().to_string()),
            ChangeTag::Delete => None,
        })
        .collect();

    println!("{}{}\n{}{}\n", "-".red(), before, "+".green(), after);
}
