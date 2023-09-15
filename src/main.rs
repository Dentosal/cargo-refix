#![feature(byte_slice_trim_ascii)]

mod args;
mod message;
mod operation;
mod selector;
mod text;

use std::{
    collections::{HashMap, HashSet},
    env,
    ffi::{OsStr, OsString},
    iter,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use clap::Parser;

fn main() {
    let mut args = env::args_os().peekable();

    // Get path to the current binary
    let bin_path_osstr = args.next().unwrap();
    let bin_path = PathBuf::from(&bin_path_osstr);
    if bin_path.file_stem() == Some(OsStr::new("cargo-refix")) {
        // Remove "refix" subcommand when called through cargo
        if args.peek() == Some(&OsString::from("refix")) {
            let _ = args.next();
        }
    }

    let args = args::Args::parse_from(iter::once(bin_path_osstr).chain(args));

    // Get path to the cargo binary
    let cargo_bin = env::var_os("CARGO").unwrap_or(OsString::from("cargo"));

    let mut cmd = Command::new(cargo_bin);
    if args.clippy {
        cmd.arg("clippy");
    } else {
        cmd.arg("check");
    }
    cmd.arg("--message-format=json");
    cmd.args(args.passthrough);

    let output = cmd.output().unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    dbg!(stderr);

    let mut list_summary: HashMap<String, HashSet<String>> = HashMap::new();

    for line in output.stdout.split(|c| *c == b'\n') {
        if line.trim_ascii().is_empty() {
            continue;
        }

        // println!("###\n{}\n###", String::from_utf8_lossy(&line));
        let msg: message::Msg = serde_json::from_slice(line).unwrap();
        if msg.reason == "compiler-message" && msg.message.as_ref().unwrap().is_singular() {
            let message = msg.message.unwrap();

            // Apply selector
            if args.selector.matches(&message) {
                if matches!(args.selector.top, selector::TopLevelSelector::List) {
                    let entry = list_summary
                        .entry(message.code().unwrap().to_owned())
                        .or_default();
                    for span in &message.spans {
                        entry.insert(span.file_name.clone());
                    }
                    continue;
                }

                args.operation.preview(&message);

                if args.single {
                    break;
                }
            }
        }
    }

    if matches!(args.selector.top, selector::TopLevelSelector::List) {
        for (code, files) in list_summary {
            print!("{}:", code);
            for file in files {
                print!(" {}", file);
            }
            println!();
        }
    }
}
