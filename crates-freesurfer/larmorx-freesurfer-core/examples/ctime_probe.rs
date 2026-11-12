// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe behind `validation/investigations/freesurfer-core/ctime_probe.py`: reads lines
//! `<TZ or -> <seconds>` on standard input and prints `ctime::localtime`'s `strftime("%c")` for
//! each (`-`: TZ unset), so that the script can compare them with the C library's.

use std::io::BufRead;

use larmorx_freesurfer_core::ctime;
use larmorx_freesurfer_core::run::Environment;

fn main() {
    for line in std::io::stdin().lock().lines() {
        let line = line.expect("a line");
        let (tz, t) = line.rsplit_once(' ').expect("<TZ> <seconds>");
        let env = if tz == "-" {
            Environment::from_pairs::<&str, &str>([])
        } else {
            Environment::from_pairs([("TZ", tz.strip_prefix('=').unwrap_or(tz))])
        };
        let t: i64 = t.parse().expect("seconds");
        println!("{}", ctime::strftime_c(&ctime::localtime(&env, t)));
    }
}
