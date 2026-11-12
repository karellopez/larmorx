// SPDX-License-Identifier: Apache-2.0
//! larmorx's own `topup` settings: the six configurations FSL ships by name (`b02b0`,
//! `b02b0_1`, `b02b0_2`, `b02b0_4`, `b02b0_7T`, `test_acqp`), a clean-room original written
//! from `specs/topup.md` §2.5's tables (the user's decision of 2026-11-09) by an implementer who
//! never opened FSL's files. Their values are FSL 6.0.7.23's (fsl-topup 2203.5): another number
//! gives another field. The text of each is a file in `settings/`, compiled in, so the
//! standalone program and the Python package find them with no FSL installation.
//!
//! `topup --config=<name>` finds a file as FSL does (as named, with `.cnf` added, in
//! `$FSLDIR/etc/flirtsch/`); a name without a directory part that none of those places holds
//! is then looked up here, as the name itself or with `.cnf` added when it has no extension
//! (`crate::cli::topup`). So an FSL installation's own file wins where FSLDIR has it, and
//! these settings serve where it does not (FSLDIR unset included, which then is not FSL-121).

/// One of the settings: its name (without `.cnf`) and its text, a configuration file of
/// `--option=value` lines and `#` comments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting {
    pub name: &'static str,
    pub text: &'static str,
}

/// The six settings, in FSL's alphabetical order.
pub const SETTINGS: &[Setting] = &[
    Setting {
        name: "b02b0",
        text: include_str!("settings/b02b0.cnf"),
    },
    Setting {
        name: "b02b0_1",
        text: include_str!("settings/b02b0_1.cnf"),
    },
    Setting {
        name: "b02b0_2",
        text: include_str!("settings/b02b0_2.cnf"),
    },
    Setting {
        name: "b02b0_4",
        text: include_str!("settings/b02b0_4.cnf"),
    },
    Setting {
        name: "b02b0_7T",
        text: include_str!("settings/b02b0_7T.cnf"),
    },
    Setting {
        name: "test_acqp",
        text: include_str!("settings/test_acqp.cnf"),
    },
];

/// The setting a `--config` name without a directory part refers to: `<name>.cnf` as given,
/// or, when `has_extension` is false, the name itself (topup adds `.cnf`). Names are matched
/// exactly (FSL's lookup on Linux, whose file names are case-sensitive).
pub fn find(name: &str, has_extension: bool) -> Option<&'static Setting> {
    SETTINGS
        .iter()
        .find(|s| name.strip_suffix(".cnf") == Some(s.name) || (!has_extension && name == s.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_as_topup_finds_them() {
        assert_eq!(find("b02b0", false).map(|s| s.name), Some("b02b0"));
        assert_eq!(find("b02b0.cnf", true).map(|s| s.name), Some("b02b0"));
        assert_eq!(find("b02b0_7T", false).map(|s| s.name), Some("b02b0_7T"));
        assert_eq!(
            find("test_acqp.cnf", true).map(|s| s.name),
            Some("test_acqp")
        );
        // another extension is not `.cnf`; the name is matched exactly
        assert_eq!(find("b02b0.txt", true), None);
        assert_eq!(find("B02B0", false), None);
        assert_eq!(find("b02b0_9", false), None);
        assert_eq!(find("b02b0.cnf.cnf", true), None);
        assert_eq!(SETTINGS.len(), 6);
    }
}
