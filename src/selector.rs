use std::str::FromStr;

use regex::Regex;

use crate::message;

#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub top: TopLevelSelector,
}

impl Selector {
    pub fn matches(&self, target: &message::CompilerMessage) -> bool {
        self.top.matches(target)
    }
}

impl FromStr for Selector {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.split(":");
        let top = s.next().unwrap().parse().unwrap();
        Ok(Self { top })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopLevelSelector {
    /// Meta selector for listing possible selectors in compact form
    List,
    /// Select all issues
    All,
    /// Error with a numeric code, such as `E0001`
    Error(u64),
    /// Named lint, such as `dead_code` or `clippy::needless_pass_by_value`
    Lint(String),
}

impl TopLevelSelector {
    pub fn matches(&self, target: &message::CompilerMessage) -> bool {
        match self {
            TopLevelSelector::List => target.code().is_some(),
            TopLevelSelector::All => target.code().is_some(),
            TopLevelSelector::Error(err) => {
                let re = Regex::new(r"^E(\d+)$").unwrap();
                target
                    .code()
                    .map(|code| {
                        if let Some(caps) = re.captures(code) {
                            caps[1].parse::<u64>().unwrap() == *err
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false)
            }
            TopLevelSelector::Lint(lint_name) => {
                target.code().map(|code| code == lint_name).unwrap_or(false)
            }
        }
    }
}

impl FromStr for TopLevelSelector {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "list" {
            return Ok(Self::List);
        } else if s == "all" {
            return Ok(Self::All);
        }

        let re = Regex::new(r"^E(\d+)$").unwrap();
        if let Some(caps) = re.captures(s) {
            Ok(Self::Error(caps[1].parse().unwrap()))
        } else {
            Ok(Self::Lint(s.to_owned()))
        }
    }
}
