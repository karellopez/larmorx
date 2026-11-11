// SPDX-License-Identifier: Apache-2.0
//! The clean-room original's command line as a program of its own, for black-box comparisons
//! before it is wired into `larmorx tedana t2smap`: `t2smap_cleanroom <t2smap's options>`.

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let settings = larmorx_tedana::cli::Settings::from_process();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = larmorx_tedana::cli::run(&args, &settings, &mut out, &mut err);
    let _ = std::io::stdout().write_all(&out);
    let _ = std::io::stderr().write_all(&err);
    std::process::exit(i32::from(code));
}
