// SPDX-License-Identifier: Apache-2.0
//! AFNI's environment variables and global options as a table of settings
//! (`specs/3dTshift.md` §7.1; `docs/porting-guide.md` §9.1).
//!
//! The process environment is never changed (`std::env::set_var` is unsafe in a
//! multithreaded program, and a changed environment leaks into everything else the process
//! runs). The command line reads it once into an [`AfniSettings`] table, applies AFNI's global
//! options to the table in command-line order ([`take_global_options`]), and passes the table
//! to every part that reads a variable. The Python API builds the table from its explicit
//! `afni_settings` argument instead ([`AfniSettings::from_pairs`]).

use larmorx_core::defects::{self, Notice, UpstreamDefects};

/// The variables 3dTshift consults that change what it writes or prints (spec §7.1's table).
pub const EFFECT_VARIABLES: &[&str] = &[
    "AFNI_DECONFLICT",
    "AFNI_ALLOW_ARBITRARY_FILENAMES",
    "AFNI_PATH_SPACES_OK",
    "HOME",
    "AFNI_NOREALPATH",
    "AFNI_NIFTI_PRIORITY",
    "AFNI_NIFTI_SCALE",
    "AFNI_NIFTI_TYPE_WARN",
    "AFNI_NIFTI_VIEW",
    "AFNI_DEFAULT_STD_SPACE",
    "AFNI_NO_OBLIQUE_WARNING",
    "AFNI_1D_ZERO_TEXT",
    "AFNI_USE_FGETS",
    "AFNI_NIFTI_WRITE_TYPE",
    "AFNI_ALLOW_MILLISECONDS",
    "AFNI_WRITE_1D_AS_PREFIX",
    "AFNI_MESSAGE_PREFIX",
    "AFNI_MESSAGE_COLORIZE",
    "AFNI_ECHO_COMMANDLINE",
    "AFNI_DONT_LOGFILE",
];

/// The variables 3dTshift consults that change nothing larmorx writes or prints (spec §7.1:
/// AFNI's header extension, its debugging output, its version banner, `.afnirc` files, its
/// random identifiers, settings without effect on NIfTI files, AFNI's table of template
/// spaces, sub-brick selectors, 1D output, AFNI's own dataset format). Accepted and ignored.
pub const NO_EFFECT_VARIABLES: &[&str] = &[
    "AFNI_NIFTI_NOEXT",
    "AFNI_HISTDB_SCRIPT",
    "AFNI_HISTORY_NAME",
    "AFNI_HISTORY_USERNAME",
    "AFNI_INCLUDE_HISTORY",
    "IDCODE_PREFIX",
    "AFNI_INDEX_PREFIX",
    "AFNI_TRACE",
    "AFNI_TRACE_FILE",
    "AFNI_NIFTI_DEBUG",
    "NIML_DEBUG",
    "AFNI_WAMI_DEBUG",
    "AFNI_DEBUG",
    "AFNI_QUIET_STARTUP",
    "AFNI_SYSTEM_AFNIRC",
    "AFNI_ENVIRON_RESET",
    "AFNI_ENVIRON_WARNINGS",
    "AFNI_RANDOM_SEEDVAL",
    "AFNI_NOMMAP",
    "AFNI_FLOATSCAN",
    "AFNI_BYTEORDER",
    "AFNI_NIML_BUFSIZE",
    "NIML_BUFSIZE",
    "BUFSIZE",
    "AFNI_ATLAS_PATH",
    "AFNI_PLUGINPATH",
    "AFNI_PLUGIN_PATH",
    "AFNI_SUPP_ATLAS",
    "AFNI_SUPP_ATLAS_DIR",
    "AFNI_LOCAL_ATLAS",
    "PATH",
    "AFNI_USE_THD_open_dataset",
    "AFNI_TCAT_SELECTORS",
    "AFNI_TCAT_SELECTORS_VERB",
    "AFNI_1D_TRANOUT",
    "AFNI_3D_BINARY",
    "AFNI_AUTOGZIP",
    "AFNI_COMPRESSOR",
    "AFNI_DONT_USE_PIGZ",
    "AFNI_WRITE_NIML",
    "AFNI_GLOB_DEBUG",
    "AFNI_GLOB_SELECTORS",
    "AFNI_SHELL_GLOB",
    "SKIP_COMPRESS_SETUP",
];

/// Whether 3dTshift consults the variable `name` (with or without an effect in larmorx).
pub fn is_consulted(name: &str) -> bool {
    EFFECT_VARIABLES.contains(&name) || NO_EFFECT_VARIABLES.contains(&name)
}

/// AFNI's *yes*: a value that starts with `Y` or `y`.
pub fn is_yes(value: Option<&str>) -> bool {
    value.is_some_and(|v| v.starts_with(['Y', 'y']))
}

/// AFNI's *no*: a value that starts with `N` or `n`.
pub fn is_no(value: Option<&str>) -> bool {
    value.is_some_and(|v| v.starts_with(['N', 'n']))
}

/// A table of AFNI's settings: variable names and values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AfniSettings {
    vars: Vec<(String, String)>,
    /// Names match whatever their case (the process environment on Windows).
    case_insensitive: bool,
    /// The global option `-overwrite` was given.
    overwrite_option: bool,
}

impl AfniSettings {
    /// An empty table: every variable unset.
    pub fn new() -> AfniSettings {
        AfniSettings::default()
    }

    /// The process environment, read once (names and values as UTF-8, invalid bytes
    /// replaced). On Windows a variable is found whatever the case of its name, as the system
    /// finds it.
    pub fn from_env() -> AfniSettings {
        AfniSettings {
            vars: std::env::vars_os()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
                .collect(),
            case_insensitive: cfg!(windows),
            overwrite_option: false,
        }
    }

    /// A table of the given settings (names case-sensitive): the Python API's
    /// `afni_settings`.
    pub fn from_pairs<I, K, V>(pairs: I) -> AfniSettings
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut s = AfniSettings::new();
        for (k, v) in pairs {
            s.set(&k.into(), &v.into());
        }
        s
    }

    fn matches(&self, a: &str, b: &str) -> bool {
        if self.case_insensitive {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    }

    /// The value of `name`, if it is set.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars
            .iter()
            .rev()
            .find(|(k, _)| self.matches(k, name))
            .map(|(_, v)| v.as_str())
    }

    /// Sets `name` to `value` (a later setting replaces an earlier one).
    pub fn set(&mut self, name: &str, value: &str) {
        let ci = self.case_insensitive;
        self.vars.retain(|(k, _)| {
            if ci {
                !k.eq_ignore_ascii_case(name)
            } else {
                k != name
            }
        });
        self.vars.push((name.to_owned(), value.to_owned()));
    }

    /// Whether `name` is *yes* (starts with `Y` or `y`).
    pub fn yes(&self, name: &str) -> bool {
        is_yes(self.get(name))
    }

    /// Whether `name` is *no* (starts with `N` or `n`).
    pub fn no(&self, name: &str) -> bool {
        is_no(self.get(name))
    }

    /// Whether the global option `-overwrite` was given.
    pub fn overwrite_option(&self) -> bool {
        self.overwrite_option
    }

    /// The table with `-overwrite` given (as [`take_global_options`] records it).
    pub fn set_overwrite_option(&mut self) {
        self.overwrite_option = true;
        self.set("AFNI_DECONFLICT", "OVERWRITE");
    }
}

/// What a variable's value does, for comparing two readings of a `-D` option (AFNI-22): two
/// values in the same class have the same effect.
fn effect(name: &str, value: Option<&str>) -> String {
    let v = value;
    match name {
        "AFNI_NIFTI_SCALE" | "AFNI_MESSAGE_COLORIZE" => format!("no={}", is_no(v)),
        "AFNI_NIFTI_TYPE_WARN" => format!("yes={} no={}", is_yes(v), is_no(v)),
        "AFNI_DECONFLICT" => format!("{:?}", Deconflict::from_value(v)),
        "AFNI_NIFTI_PRIORITY" => match v {
            None => "S".into(),
            Some(s) if s.starts_with(['S', 's']) => "S".into(),
            Some(s) if s.starts_with(['Q', 'q']) => "Q".into(),
            Some(s) => format!("illegal {s}"),
        },
        "AFNI_NIFTI_VIEW" => {
            let view = v.map(str::to_ascii_lowercase);
            match view.as_deref() {
                Some(w @ ("tlrc" | "orig" | "acpc")) => w.to_owned(),
                _ => "orig".into(),
            }
        }
        "AFNI_NIFTI_WRITE_TYPE" => format!("{}", write_type(v)),
        "HOME" | "AFNI_MESSAGE_PREFIX" | "AFNI_DEFAULT_STD_SPACE" => format!("{v:?}"),
        _ => format!("yes={}", is_yes(v)),
    }
}

/// `AFNI_NIFTI_WRITE_TYPE` as AFNI reads it: a number with an optional multiplier (`k`, `m`,
/// `g`: 1024, 1024², 1024³), cut to an integer; 0 when unset or not a number.
pub fn write_type(value: Option<&str>) -> i64 {
    let Some(v) = value else { return 0 };
    let Some((x, rest)) = crate::cli::leading_number(v) else {
        return 0;
    };
    let scale = match rest.chars().next() {
        Some('k' | 'K') => 1024.0,
        Some('m' | 'M') => 1024.0 * 1024.0,
        Some('g' | 'G') => 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    let x = x * scale;
    if x.is_finite() {
        x.trunc().clamp(i64::MIN as f64, i64::MAX as f64) as i64
    } else {
        0
    }
}

/// `AFNI_DECONFLICT`: what happens to an existing output (spec §7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deconflict {
    /// Unset, or a value not starting with `O`, `o`, `Y`, `y` or `N`: AFNI-11.
    Unset,
    /// `OVERWRITE` exactly (what `-overwrite` sets): replaced, with AFNI's warning unless
    /// `-overwrite` was given.
    OverwriteExact,
    /// Another value starting with `O` or `o`: replaced, without a warning.
    Overwrite,
    /// Starting with `Y` or `y`: the output is renamed (`out_AA1.nii`, ...).
    Rename,
    /// Starting with upper-case `N`: a shift stops before the work (a copy: as unset).
    Stop,
}

impl Deconflict {
    pub fn from_value(value: Option<&str>) -> Deconflict {
        match value {
            Some("OVERWRITE") => Deconflict::OverwriteExact,
            Some(v) if v.starts_with(['O', 'o']) => Deconflict::Overwrite,
            Some(v) if v.starts_with(['Y', 'y']) => Deconflict::Rename,
            Some(v) if v.starts_with('N') => Deconflict::Stop,
            _ => Deconflict::Unset,
        }
    }

    pub fn from_settings(settings: &AfniSettings) -> Deconflict {
        Deconflict::from_value(settings.get("AFNI_DECONFLICT"))
    }
}

/// What the global options took out of a command line.
#[derive(Debug, Default)]
pub struct GlobalOptions {
    /// The arguments left, in order.
    pub rest: Vec<String>,
    /// AFNI-22 notices.
    pub notices: Vec<Notice>,
    /// larmorx's own warnings (each ends in `(larmorx)`).
    pub warnings: Vec<String>,
}

/// C's `isspace` in the C locale.
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `sscanf`'s `%<max>s` from byte `start`: white space skipped, then at most `max`
/// non-blank bytes. `None` if there is no word.
fn scan_word(text: &[u8], start: usize, max: usize) -> Option<(Vec<u8>, usize)> {
    let mut i = start;
    while i < text.len() && is_c_space(text[i]) {
        i += 1;
    }
    let begin = i;
    while i < text.len() && !is_c_space(text[i]) && i - begin < max {
        i += 1;
    }
    (i > begin).then(|| (text[begin..i].to_vec(), i))
}

/// `sscanf(text, "%255s %1023s")`: the two words found, if any.
fn scan_two(text: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    match scan_word(text, 0, 255) {
        None => (None, None),
        Some((first, end)) => (Some(first), scan_word(text, end, 1023).map(|(w, _)| w)),
    }
}

/// What AFNI sets for `-D<text>` (spec §7.1): the text read as two words separated by blanks
/// (the first `=` made a blank if either is missing), joined by `=`, then split at the first
/// `=` into a name and a value. `None` when it sets nothing.
pub fn afni_reading(text: &str) -> Option<(String, String)> {
    let bytes = text.as_bytes();
    if bytes.len() < 3 {
        return None;
    }
    let (first, second) = match scan_two(bytes) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            let mut t = bytes.to_vec();
            if let Some(i) = t.iter().position(|&b| b == b'=') {
                t[i] = b' ';
            }
            let (a, b) = scan_two(&t);
            (a.unwrap_or_default(), b.unwrap_or_default())
        }
    };
    if first.is_empty() || second.is_empty() {
        return None;
    }
    let mut joined = first;
    joined.push(b'=');
    joined.extend_from_slice(&second);
    let joined = String::from_utf8_lossy(&joined).into_owned();
    let (name, value) = joined.split_once('=').expect("joined by '='");
    Some((name.to_owned(), value.to_owned()))
}

/// What larmorx's default sets for `-D<text>`: the name before the first `=` and the value
/// after it, as written. `None` when either is empty.
pub fn as_written(text: &str) -> Option<(String, String)> {
    let (name, value) = text.split_once('=')?;
    (!name.is_empty() && !value.is_empty()).then(|| (name.to_owned(), value.to_owned()))
}

/// Whether `arg` is AFNI's global option `-Dname=value`.
pub fn is_define(arg: &str) -> bool {
    arg.starts_with("-D") && arg.contains('=')
}

/// Takes AFNI's global options (`-overwrite`, `-Dname=value`, `-ok_1D_text`, `-skip_afnirc`)
/// out of `args`, wherever they stand, and applies them to `settings` in order (spec §7.1).
/// `-D` values are read as written by default and as AFNI reads them with
/// [`UpstreamDefects::Reproduce`] (AFNI-22, named where the two readings differ in effect).
pub fn take_global_options(
    args: &[String],
    settings: &mut AfniSettings,
    mode: UpstreamDefects,
) -> GlobalOptions {
    let mut g = GlobalOptions::default();
    for arg in args {
        match arg.as_str() {
            "-overwrite" => settings.set_overwrite_option(),
            "-ok_1D_text" => settings.set("AFNI_1D_ZERO_TEXT", "YES"),
            "-skip_afnirc" => {}
            a if is_define(a) => define(a, settings, mode, &mut g),
            _ => g.rest.push(arg.clone()),
        }
    }
    g
}

/// Applies one `-Dname=value`.
fn define(arg: &str, settings: &mut AfniSettings, mode: UpstreamDefects, g: &mut GlobalOptions) {
    let text = &arg[2..];
    let afni = afni_reading(text);
    let ours = as_written(text);
    let chosen = if mode.reproduce() { &afni } else { &ours };
    // AFNI-22: named when the two readings would make a variable of the table act differently.
    let mut names: Vec<&str> = [&afni, &ours]
        .into_iter()
        .flatten()
        .map(|(n, _)| n.as_str())
        .filter(|n| EFFECT_VARIABLES.contains(n))
        .collect();
    names.dedup();
    let differs = names.iter().any(|&n| {
        let current = settings.get(n);
        let value = |r: &Option<(String, String)>| match r {
            Some((m, v)) if m == n => Some(v.clone()),
            _ => current.map(str::to_owned),
        };
        effect(n, value(&afni).as_deref()) != effect(n, value(&ours).as_deref())
    });
    if differs {
        let describe = |r: &Option<(String, String)>| match r {
            Some((n, v)) => format!("sets {n} to '{v}'"),
            None => "sets nothing".to_owned(),
        };
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "'{arg}' {}; AFNI's reading {}",
                describe(&ours),
                describe(&afni)
            ),
            UpstreamDefects::Reproduce => format!("'{arg}' {}, as AFNI reads it", describe(&afni)),
        };
        g.notices.push(Notice::new(&defects::AFNI_22, mode, detail));
    }
    match chosen {
        Some((name, value)) => {
            if !is_consulted(name) {
                g.warnings.push(format!(
                    "{arg} has no effect: 3dTshift does not read {name} (larmorx)"
                ));
            }
            settings.set(name, value);
        }
        None => {
            if mode.fix() {
                g.warnings.push(format!(
                    "{arg} sets nothing (a name and a value are needed), as in AFNI (larmorx)"
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn afni_reads_d_options_as_two_words() {
        let r = |t: &str| afni_reading(t).map(|(n, v)| format!("{n}={v}"));
        // Spec §7.1's table (observed).
        assert_eq!(r("X=YES").as_deref(), Some("X=YES"));
        assert_eq!(r("X=YES=1").as_deref(), Some("X=YES=1"));
        assert_eq!(r("X=a b").as_deref(), Some("X=a=b"));
        assert_eq!(r("X= YES").as_deref(), Some("X==YES"));
        assert_eq!(r("X =YES").as_deref(), Some("X==YES"));
        assert_eq!(r("X="), None);
        assert_eq!(r("=YES"), None);
        assert_eq!(r("AFNI_ALLOW_ARBITRARY_FILENAMES="), None);
        assert_eq!(
            r("AFNI_ALLOW_ARBITRARY_FILENAMES=Y ES").as_deref(),
            Some("AFNI_ALLOW_ARBITRARY_FILENAMES=Y=ES")
        );
        // A first word longer than 255 characters continues as the second.
        let long = format!("{}=v", "N".repeat(300));
        let (name, value) = afni_reading(&long).unwrap();
        assert_eq!(name.len(), 255);
        assert_eq!(value, format!("{}=v", "N".repeat(45)));
    }

    #[test]
    fn larmorx_reads_them_as_written() {
        assert_eq!(as_written("X=a b"), Some(("X".into(), "a b".into())));
        assert_eq!(as_written("X= YES"), Some(("X".into(), " YES".into())));
        assert_eq!(as_written("X=YES=1"), Some(("X".into(), "YES=1".into())));
        assert_eq!(as_written("X="), None);
        assert_eq!(as_written("=YES"), None);
    }

    #[test]
    fn global_options_are_taken_out_wherever_they_stand() {
        let mut s = AfniSettings::from_pairs([("AFNI_DECONFLICT", "YES")]);
        let g = take_global_options(
            &args(&[
                "-tpattern",
                "-overwrite",
                "alt+z",
                "-DAFNI_NIFTI_VIEW=tlrc",
                "in.nii",
                "-ok_1D_text",
                "-skip_afnirc",
            ]),
            &mut s,
            UpstreamDefects::Fix,
        );
        assert_eq!(g.rest, args(&["-tpattern", "alt+z", "in.nii"]));
        assert!(s.overwrite_option());
        assert_eq!(s.get("AFNI_DECONFLICT"), Some("OVERWRITE"));
        assert_eq!(s.get("AFNI_NIFTI_VIEW"), Some("tlrc"));
        assert!(s.yes("AFNI_1D_ZERO_TEXT"));
        assert!(g.notices.is_empty() && g.warnings.is_empty());
        // A later setting wins; -D wins over the environment.
        let mut s = AfniSettings::from_pairs([("AFNI_DECONFLICT", "YES")]);
        take_global_options(
            &args(&["-overwrite", "-DAFNI_DECONFLICT=NO"]),
            &mut s,
            UpstreamDefects::Fix,
        );
        assert_eq!(Deconflict::from_settings(&s), Deconflict::Stop);
        // Not global options: -D without '=', lower-case -d.
        let g = take_global_options(&args(&["-DX", "-dX=1", "-D"]), &mut s, UpstreamDefects::Fix);
        assert_eq!(g.rest, args(&["-DX", "-dX=1", "-D"]));
    }

    #[test]
    fn afni_22_is_named_where_the_readings_act_differently() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut s = AfniSettings::new();
            let g = take_global_options(&args(&["-DAFNI_MESSAGE_PREFIX=a b"]), &mut s, mode);
            assert_eq!(g.notices.len(), 1);
            assert_eq!(g.notices[0].defect.id, "AFNI-22");
            let expected = if mode.fix() { "a b" } else { "a=b" };
            assert_eq!(s.get("AFNI_MESSAGE_PREFIX"), Some(expected));
            // Yes either way: nothing named.
            let mut s = AfniSettings::new();
            let g = take_global_options(
                &args(&["-DAFNI_ALLOW_ARBITRARY_FILENAMES=Y ES"]),
                &mut s,
                mode,
            );
            assert!(g.notices.is_empty());
            assert!(s.yes("AFNI_ALLOW_ARBITRARY_FILENAMES"));
        }
    }

    #[test]
    fn unknown_and_empty_definitions() {
        let mut s = AfniSettings::new();
        let g = take_global_options(
            &args(&[
                "-DAFNI_NOT_A_VARIABLE=1",
                "-DAFNI_NIFTI_NOEXT=YES",
                "-DX=",
                "-D=Y",
            ]),
            &mut s,
            UpstreamDefects::Fix,
        );
        assert_eq!(s.get("AFNI_NOT_A_VARIABLE"), Some("1"));
        assert_eq!(g.warnings.len(), 3, "{:?}", g.warnings);
        assert!(g.warnings[0].contains("does not read AFNI_NOT_A_VARIABLE (larmorx)"));
        assert!(g.warnings[1].starts_with("-DX= sets nothing"));
        // AFNI's reading says nothing about an empty -D.
        let g = take_global_options(&args(&["-DX="]), &mut s, UpstreamDefects::Reproduce);
        assert!(g.warnings.is_empty());
    }

    #[test]
    fn deconflict_values() {
        let d = |v: &str| Deconflict::from_value(Some(v));
        assert_eq!(d("OVERWRITE"), Deconflict::OverwriteExact);
        assert_eq!(d("overwrite"), Deconflict::Overwrite);
        assert_eq!(d("O"), Deconflict::Overwrite);
        assert_eq!(d("YES"), Deconflict::Rename);
        assert_eq!(d("yes"), Deconflict::Rename);
        assert_eq!(d("NO"), Deconflict::Stop);
        assert_eq!(d("no"), Deconflict::Unset);
        assert_eq!(d("1"), Deconflict::Unset);
        assert_eq!(Deconflict::from_value(None), Deconflict::Unset);
    }

    #[test]
    fn write_types() {
        assert_eq!(write_type(Some("2")), 2);
        assert_eq!(write_type(Some("2.9")), 2);
        assert_eq!(write_type(Some("2k")), 2048);
        assert_eq!(write_type(Some("1")), 1);
        assert_eq!(write_type(Some("abc")), 0);
        assert_eq!(write_type(None), 0);
    }

    #[test]
    fn names_on_windows_match_whatever_their_case() {
        let mut s = AfniSettings {
            case_insensitive: true,
            ..AfniSettings::new()
        };
        s.set("afni_nifti_view", "tlrc");
        assert_eq!(s.get("AFNI_NIFTI_VIEW"), Some("tlrc"));
        s.set("AFNI_NIFTI_VIEW", "orig");
        assert_eq!(s.get("afni_nifti_view"), Some("orig"));
        let t = AfniSettings::from_pairs([("afni_nifti_view", "tlrc")]);
        assert_eq!(t.get("AFNI_NIFTI_VIEW"), None);
    }
}
