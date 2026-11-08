// SPDX-License-Identifier: Apache-2.0 AND PSF-2.0
//! CPython 3.12.16's behaviours that a ported Python program shows on its command line and in
//! its messages, reproduced exactly, so that a replica parses, refuses and prints as the
//! original does:
//!
//! - [`argparse`]: `argparse.ArgumentParser` for optional arguments (options with `nargs`
//!   `None`, `'?'`, `'*'`, `'+'` or a count, `store`, `store_true`, `help` and `version`
//!   actions, `type` conversions, `choices`, `required`, abbreviations, `--opt=value`, clusters
//!   of short options, `--`), its usage and help text (`HelpFormatter` and
//!   `ArgumentDefaultsHelpFormatter`, wrapped at the terminal width as `shutil` finds it) and its
//!   error messages and exit statuses.
//! - [`textwrap`]: `textwrap.wrap` with its default options (argparse wraps help text with it).
//! - [`number`]: `float(str)` and `int(str)` as Python parses them, and `repr(float)`.
//! - [`json`]: `json.dump` with an indent (`Lib/json/encoder.py`).
//!
//! Upstream: CPython 3.12.16 `Lib/argparse.py`, `Lib/json/encoder.py`, `Lib/textwrap.py`, `Lib/shutil.py`
//! (`get_terminal_size`), `Objects/floatobject.c`, `Objects/longobject.c`, `Python/pystrtod.c`
//! (PSF License Version 2; "Copyright (c) 2001-2024 Python Software Foundation; All Rights
//! Reserved"; the licence text in NOTICE). The behaviour is re-expressed in Rust; no code was
//! copied. Positional arguments, subparsers, mutually exclusive groups and `fromfile` prefixes
//! are not ported (no program ported so far uses them).

pub mod argparse;
pub mod json;
pub mod number;
pub mod textwrap;
