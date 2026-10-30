// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl bet`: the `bet` script's command line and variations (`specs/bet.md` §5).

use std::io::Write;

use super::Settings;

/// Runs the `bet` script.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let _ = (args, settings, out, err);
    1
}
