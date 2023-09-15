use std::ffi::OsString;

use clap::Parser;

use crate::{operation::Operation, selector::Selector};

/// Automation helper to fix rust errors and warnings
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Allow applying fixes to a uncommitted working tree
    #[arg(short = 'd', long)]
    pub allow_dirty: bool,

    /// Stop after first match
    #[arg(short, long)]
    pub single: bool,

    /// Actually apply changes instead of just previewing
    #[arg(long)]
    pub write: bool,

    /// Run clippy in addition to check
    #[arg(short, long)]
    pub clippy: bool,

    /// Selector for issue category to fix
    pub selector: Selector,

    /// Operation to apply to the selected issues
    #[clap(flatten)]
    pub operation: Operation,

    /// Passthrough arguments to cargo check/clippy, using -- to separate
    #[clap(last = true)]
    pub passthrough: Vec<OsString>,
}
