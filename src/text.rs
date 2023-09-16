use std::ops;

use colored::Colorize;
use regex::Regex;

use crate::operation::ExecError;

pub fn underline_span(text: &str, span: ops::Range<usize>) -> String {
    format!(
        "{}{}{}",
        &text[..span.start],
        &text[span.start..span.end].underline(),
        &text[span.end..],
    )
}

const PARENS: [(char, char); 4] = [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];

/// Returns tuple (other paren, this paren is opening)
fn other_paren(paren: char) -> Option<(char, bool)> {
    if let Some((_, other)) = PARENS.iter().find(|(l, _)| *l == paren) {
        Some((*other, true))
    } else if let Some((other, _)) = PARENS.iter().find(|(_, r)| *r == paren) {
        Some((*other, false))
    } else {
        None
    }
}

pub fn find_matching_paren(context: &str, index: usize) -> Option<usize> {
    if !context.is_char_boundary(index) {
        panic!("Expected index to be char boundary: index={index}");
    }
    if !context.is_char_boundary(index + 1) {
        panic!("Expected index+1 to be char boundary: index={index}");
    }
    let paren = context[index..index + 1].chars().next().unwrap();

    let (end_paren, opening) = other_paren(paren)?;

    let mut stack = vec![end_paren];
    if opening {
        for (i, c) in context[index + 1..].char_indices() {
            if let Some((other, opens)) = other_paren(c) {
                if opens {
                    stack.push(other);
                } else if let Some(top) = stack.pop() {
                    if top != c {
                        return None; // Mismatched paren type
                    }
                    if stack.is_empty() {
                        return Some(index + 1 + i);
                    }
                } else {
                    return None; // Mismatched paren count
                }
            }
        }
        return None; // No closing paren in scope
    } else {
        for (i, c) in context[..index].char_indices().rev() {
            if let Some((other, opens)) = other_paren(c) {
                if !opens {
                    stack.push(other);
                } else if let Some(top) = stack.pop() {
                    if top != c {
                        return None; // Mismatched paren type
                    }
                    if stack.is_empty() {
                        return Some(i);
                    }
                } else {
                    return None; // Mismatched paren count
                }
            }
        }
        return None; // No opening paren in scope
    }
}

/// Replaces templates in form `$name` or `${name}`, using a resolver function.
/// If resolver returns `Ok(None)`, the template is left as-is.
pub fn template<F>(template: &str, mut resolver: F) -> Result<String, ExecError>
where
    F: FnMut(&str) -> Result<Option<String>, ExecError>,
{
    let re = Regex::new(r"\$([A-Za-z][A-Za-z0-9_]*)|\$\{([^\}]+)\}").unwrap();
    let mut replacements = Vec::new();
    for m in re.captures_iter(&template) {
        let value = m.get(1).or(m.get(2)).unwrap().as_str();
        if let Some(replacement) = resolver(&value)? {
            replacements.push((m.get(0).unwrap().range(), replacement));
        }
    }

    let mut result = template.to_owned();
    while let Some((range, replacement)) = replacements.pop() {
        result.replace_range(range, &replacement);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use crate::{
        operation::{ExecError, TextOperation},
        text::template,
    };

    use super::find_matching_paren;

    #[test]
    fn test_find_matching_paren() {
        assert_eq!(find_matching_paren("()", 0), Some(1));
        assert_eq!(find_matching_paren("()", 1), Some(0));
        assert_eq!(find_matching_paren("x()", 0), None);
        assert_eq!(find_matching_paren("<()>", 0), Some(3));
        assert_eq!(find_matching_paren("<()>", 1), Some(2));
        assert_eq!(find_matching_paren("<()>", 2), Some(1));
        assert_eq!(find_matching_paren("<()>", 3), Some(0));
        assert_eq!(find_matching_paren("<([{}])>", 0), Some(7));
        assert_eq!(find_matching_paren("<([{}])>", 7), Some(0));
        assert_eq!(find_matching_paren("<a(b[c{d}e]f)g>", 0), Some(14));
        assert_eq!(find_matching_paren("<a(b[c{d}e]f)g>", 14), Some(0));
        assert_eq!(find_matching_paren("<a(b[c{d}e]f)g>", 2), Some(12));
        assert_eq!(find_matching_paren("<a(b[c{d}e]f)g>", 12), Some(2));
    }

    #[test]
    fn test_template() {
        fn increment_a(a: &str) -> Result<Option<String>, ExecError> {
            // Only do something to a-prefixed names
            let Some(a) = a.strip_prefix('a') else {
                return Ok(None);
            };
            let a = a
                .parse::<u64>()
                .expect("Invalid integer in template variable");
            Ok(Some((a + 1).to_string()))
        }

        assert_eq!(template("$a123", increment_a).unwrap(), "124".to_owned());
        assert_eq!(template("${a0}", increment_a).unwrap(), "1".to_owned());
        assert_eq!(
            template("XX$a123 XX", increment_a).unwrap(),
            "XX124 XX".to_owned()
        );
        assert_eq!(
            template("XX${a0}XX", increment_a).unwrap(),
            "XX1XX".to_owned()
        );
        assert_eq!(
            template("12${a2}45", increment_a).unwrap(),
            "12345".to_owned()
        );
        assert_eq!(
            template("$${a2}$$", increment_a).unwrap(),
            "$3$$".to_owned()
        );
        assert_eq!(template("${b2}", increment_a).unwrap(), "${b2}".to_owned());
    }
}
