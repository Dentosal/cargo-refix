#![feature(byte_slice_trim_ascii)]

mod args;
mod message;
mod operation;
mod selector;
mod text;

use std::{
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
    if bin_path.file_stem() == Some(OsStr::new("cargo-pfix")) {
        // Remove "pfix" subcommand when called through cargo
        if args.peek() == Some(&OsString::from("pfix")) {
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

    for line in output.stdout.split(|c| *c == b'\n') {
        if line.trim_ascii().is_empty() {
            continue;
        }

        // println!("###\n{}\n###", String::from_utf8_lossy(&line));
        let msg: message::Msg = serde_json::from_slice(line).unwrap();
        if msg.reason == "compiler-message" && msg.message.as_ref().unwrap().is_singular() {
            // Apply selector
            if args.selector.matches(msg.message.as_ref().unwrap()) {
                args.operation.preview(msg.message.as_ref().unwrap());

                if args.single {
                    break;
                }
            }
        }
    }
}
