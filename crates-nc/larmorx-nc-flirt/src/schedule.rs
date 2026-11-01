// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (defaultschedule.h; flirt.cc:
// stripleadingspace, striptrailingspace, parseline, setmatvariable, setscalarvariable,
// parsematname). Copyright (C) 1999-2023 University of Oxford (Mark Jenkinson). Rust translation
// Copyright 2026 Karel Lopez Vilaret; modified from the original. Distributed under the FSL
// Licence (see LICENSE).
//! FLIRT's schedules: the default 3D and 2D schedules, and the parsing of a schedule line (its
//! words, matrix names with row ranges, scalar arguments).

/// `setdefaultschedule(comms)`.
pub const DEFAULT_3D: &[&str] = &[
    "# 8mm scale",
    "setscale 8",
    "setoption smoothing 8",
    "clear S",
    "clear P",
    "search",
    "# 4mm scale",
    "setscale 4",
    "setoption smoothing 4",
    "clear U",
    "clear UA ",
    "clear UB",
    "clear US",
    "clear UP",
    "# remeasure costs at this scale",
    "measurecost 7 S 0 0 0 0 0 0 rel",
    "copy U US",
    "clear U",
    "measurecost 7 P 0 0 0 0 0 0 rel",
    "copy U UP",
    "dualsort US UP",
    "# optimise best 3 candidates (pre and post 8mm optimisations)",
    "clear U",
    "optimise 7 US:1-3  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "optimise 7 UP:1-3  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "# also try the identity transform as a starting point at this resolution",
    "clear UQ",
    "setrow UQ  1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1",
    "optimise 7 UQ  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "sort U",
    "copy U UA",
    "# select best 4 optimised solutions and try perturbations of these",
    "clear U",
    "copy UA:1-4 U",
    "optimise 7 UA:1-4  1.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "optimise 7 UA:1-4 -1.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "optimise 7 UA:1-4  0.0   1.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "optimise 7 UA:1-4  0.0  -1.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "optimise 7 UA:1-4  0.0   0.0   1.0   0.0   0.0   0.0   0.0  rel 4",
    "optimise 7 UA:1-4  0.0   0.0  -1.0   0.0   0.0   0.0   0.0  rel 4",
    "optimise 7 UA:1-4  0.0   0.0   0.0   0.0   0.0   0.0   0.1  abs 4",
    "optimise 7 UA:1-4  0.0   0.0   0.0   0.0   0.0   0.0  -0.1  abs 4",
    "optimise 7 UA:1-4  0.0   0.0   0.0   0.0   0.0   0.0   0.2  abs 4",
    "optimise 7 UA:1-4  0.0   0.0   0.0   0.0   0.0   0.0  -0.2  abs 4",
    "sort U",
    "copy U UB",
    "# 2mm scale",
    "setscale 2",
    "setoption smoothing 2",
    "clear U",
    "clear UC",
    "clear UD",
    "clear UE",
    "clear UF",
    "# remeasure costs at this scale",
    "measurecost 7 UB 0 0 0 0 0 0 rel",
    "sort U",
    "copy U UC",
    "clear U",
    "optimise 7  UC:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "copy U UD",
    "setoption boundguess 1",
    "if MAXDOF > 7",
    " clear U",
    "if MAXDOF > 7",
    " optimise 9  UD:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1",
    "copy U UE",
    "if MAXDOF > 9",
    " clear U",
    "if MAXDOF > 9",
    " optimise 12 UE:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 2",
    "sort U",
    "copy U UF",
    "# 1mm scale",
    "setscale 1",
    "setoption smoothing 1",
    "setoption boundguess 1",
    "clear U",
    "# also try the qsform as a starting point at this resolution",
    "setrowqsform UF",
    "optimise 12 UF:1-2  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1",
    "# in addition, try qsform as the final transformation, not just an initialisation",
    "clear UG",
    "setrowqsform UG",
    "measurecost 12 UG:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1",
    "sort U",
];

/// `set2Ddefaultschedule(comms)`.
pub const DEFAULT_2D: &[&str] = &[
    "# 8mm scale",
    "setscale 8",
    "setoption smoothing 8",
    "setoption paramsubset 3  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0",
    "clear U",
    "clear UA",
    "setrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1",
    "optimise 12 UA:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4 ",
    "",
    "# 4mm scale",
    "setscale 4",
    "setoption smoothing 4",
    "setoption paramsubset 3  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0",
    "clear UB",
    "clear UL",
    "clear UM",
    "# remeasure costs at this scale",
    "clear U",
    "measurecost 12 UA 0 0 0 0 0 0 rel",
    "sort U",
    "copy U UL",
    "# optimise best 3 candidates",
    "clear U",
    "optimise 12 UL:1-3  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "# also try the identity transform as a starting point at this resolution",
    "clear UQ",
    "setrow UQ  1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1",
    "optimise 7 UQ  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "sort U",
    "copy U UM",
    "# select best 4 optimised solutions and try perturbations of these",
    "clear U",
    "copy UM:1-4 U",
    "optimise 12 UM:1-4  0.0   0.0   1.0   0.0   0.0   0.0   0.0  rel 4",
    "optimise 12 UM:1-4  0.0   0.0  -1.0   0.0   0.0   0.0   0.0  rel 4",
    "sort U",
    "clear UB",
    "copy U UB",
    "",
    "# 2mm scale",
    "setscale 2",
    "setoption smoothing 2",
    "setoption paramsubset 3  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0",
    "clear U",
    "clear UC",
    "clear UD",
    "clear UE",
    "clear UF",
    "# remeasure costs at this scale",
    "measurecost 12 UB 0 0 0 0 0 0 rel",
    "sort U",
    "copy U UC",
    "clear U",
    "optimise 12  UC:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4",
    "copy U UD",
    "setoption boundguess 1",
    "if MAXDOF > 7",
    " clear U",
    "if MAXDOF > 7",
    " optimise 9  UD:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1",
    "copy U UE",
    "if MAXDOF > 9",
    " clear U",
    "if MAXDOF > 9",
    " optimise 12 UE:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 2",
    "sort U",
    "copy U UF",
    "",
    "# 1mm scale",
    "setscale 1",
    "setoption smoothing 1",
    "setoption boundguess 1",
    "setoption paramsubset 3  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0",
    "clear U",
    "# also try the qsform as a starting point at this resolution",
    "setrowqsform UF",
    "optimise 12 UF:1-2  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1",
    "sort U",
];

/// `stripleadingspace(line)`: blanks and tabs removed at the start.
fn strip_leading(s: &str) -> &str {
    s.trim_start_matches([' ', '\t'])
}

/// `striptrailingspace(line)`: blanks and tabs removed at the end; a line of blanks becomes
/// empty. (FSL keeps the line when the last non-blank character is its first: `find_last_not_of`
/// returning 0 is treated as "nothing to strip", which leaves `"x  "` as it is.)
fn strip_trailing(s: &str) -> &str {
    match s.rfind(|c| c != ' ' && c != '\t') {
        None => "",
        Some(0) => s,
        Some(i) => &s[..=i],
    }
}

/// `parseline(inname, words)`: the line's words, split at blanks and tabs after the ends are
/// stripped, as FSL splits them.
pub fn parseline(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut name = line.to_owned();
    while !name.is_empty() {
        let t = strip_trailing(strip_leading(&name)).to_owned();
        name = t;
        let i1 = name.find([' ', '\t']);
        match i1 {
            Some(i) => {
                words.push(name[..i].to_owned());
                name.replace_range(..i, "");
            }
            None => {
                words.push(name.clone());
                name.clear();
            }
        }
    }
    words
}

/// A matrix variable of a schedule: `S` (the search's results), `P` (their pre-optimised
/// starts), `U` and `UA` to `UZ` (`usrmat[0..=26]`; `U` followed by a digit is `U`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatName {
    S,
    P,
    U(usize),
}

/// Why a matrix name could not be read; FSL then uses an uninitialised pointer (undefined
/// behaviour, refused here).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadName {
    /// `setmatvariable` returned without a message (an empty name).
    Empty,
    /// What FSL printed on standard error.
    Message(String),
    /// `U[` is `usrmat[27]`, one past the 27 matrices (read and written out of bounds).
    OutOfRange(String),
}

/// `setmatvariable(name, namedmat)`.
pub fn setmatvariable(name: &str) -> Result<MatName, BadName> {
    if name.is_empty() {
        return Err(BadName::Empty);
    }
    match name {
        "S" => return Ok(MatName::S),
        "P" => return Ok(MatName::P),
        "U" => return Ok(MatName::U(0)),
        _ => {}
    }
    let b = name.as_bytes();
    if b[0] == b'U' {
        let second = b.get(1).copied().unwrap_or(0);
        // name[1] - 'A' + 1 with chars promoted to int.
        let idx = i32::from(second as i8) - i32::from(b'A') + 1;
        if !(1..=27).contains(&idx) {
            if !second.is_ascii_digit() {
                return Err(BadName::Message(format!(
                    "Can only accept UA to UZ, not {name}"
                )));
            }
            return Ok(MatName::U(0));
        }
        if idx == 27 {
            return Err(BadName::OutOfRange(name.to_owned()));
        }
        return Ok(MatName::U(idx as usize));
    }
    Err(BadName::Message(format!(
        "Cannot interpret {name} as a Matrix name"
    )))
}

/// The scalar variables a schedule can name.
#[derive(Clone, Copy, Debug)]
pub struct Scalars {
    pub maxdof: i32,
    pub minsampling: f32,
    pub verbose: i32,
}

/// `setscalarvariable(name, float&)`: `MAXDOF`, `MINSAMPLING`, a name starting with a letter (0,
/// a message at verbose above 20, status -1), or `atof` of the word. Returns the value and the
/// message, if any.
pub fn scalar_f(name: &str, s: &Scalars) -> (f32, Option<String>) {
    if name.is_empty() {
        return (0.0, None);
    }
    match name {
        "MAXDOF" => return (s.maxdof as f32, None),
        "MINSAMPLING" => return (s.minsampling, None),
        _ => {}
    }
    if name.as_bytes()[0].is_ascii_alphabetic() {
        let msg =
            (s.verbose > 20).then(|| format!("Cannot interpret {name} as a scalar variable name"));
        return (0.0, msg);
    }
    (larmorx_nc_core::cnum::atof(name) as f32, None)
}

/// `setscalarvariable(name, int&)`: the float value rounded (`MISCMATHS::round`; an empty name
/// leaves the float at 0, so the integer becomes 0).
pub fn scalar_i(name: &str, s: &Scalars, _current: i32) -> (i32, Option<String>) {
    let (v, msg) = scalar_f(name, s);
    (larmorx_nc_core::interp::round_f32(v), msg)
}

/// `parsematname(inname, usrdefmat, r1, r2)`: `NAME[:r1[-r2]]` (rows 1-based; `r2` defaults to
/// 999999 or to `r1` when only `r1` is given; `r1` at least 1, `r2` at least `r1`). An empty
/// name returns `Ok(None)` with the rows unchanged (FSL leaves the pointer unset).
#[allow(clippy::type_complexity)]
pub fn parsematname(
    inname: &str,
    s: &Scalars,
) -> (Result<Option<MatName>, BadName>, i32, i32, Vec<String>) {
    let name = strip_trailing(strip_leading(inname));
    let mut msgs = Vec::new();
    if name.is_empty() {
        return (Ok(None), 0, 0, msgs);
    }
    let colon = name.find(':');
    let hyphen = name.find('-');
    let basename = match colon {
        Some(c) => &name[..c],
        None => name,
    };
    let (mut row1, mut row2) = (String::new(), String::new());
    match (colon, hyphen) {
        (Some(c), Some(h)) => {
            // substr(colon+1, hyphen-colon-1): an empty or wrapped length when the hyphen comes
            // first.
            let start = c + 1;
            row1 = if h >= start {
                name[start..h].to_owned()
            } else {
                name[start..].to_owned()
            };
            row2 = name[h + 1..].to_owned();
        }
        (Some(c), None) => {
            row1 = name[c + 1..].to_owned();
            row2 = row1.clone();
        }
        _ => {}
    }
    if row1.is_empty() {
        row1 = "1".to_owned();
    }
    if row2.is_empty() {
        row2 = "999999".to_owned();
    }
    let m = setmatvariable(basename);
    if let Err(BadName::Message(t)) = &m {
        msgs.push(t.clone());
    }
    let (mut r1, m1) = scalar_i(&row1, s, 0);
    let (mut r2, m2) = scalar_i(&row2, s, 0);
    msgs.extend(m1);
    msgs.extend(m2);
    if r1 < 1 {
        r1 = 1;
    }
    if r2 < r1 {
        r2 = r1;
    }
    (m.map(Some), r1, r2, msgs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_as_fsl_splits_them() {
        assert_eq!(parseline("clear UA "), vec!["clear", "UA"]);
        assert_eq!(
            parseline("optimise 7 US:1-3  0.0\t0.0"),
            vec!["optimise", "7", "US:1-3", "0.0", "0.0"]
        );
        assert_eq!(parseline("   "), vec![""]);
        assert_eq!(parseline("clear U  "), vec!["clear", "U", ""]);
        assert_eq!(parseline(" if MAXDOF > 7"), vec!["if", "MAXDOF", ">", "7"]);
    }

    #[test]
    fn matrix_names() {
        let s = Scalars {
            maxdof: 12,
            minsampling: 1.0,
            verbose: 0,
        };
        assert_eq!(setmatvariable("UZ"), Ok(MatName::U(26)));
        assert_eq!(setmatvariable("U7"), Ok(MatName::U(0)));
        assert!(matches!(setmatvariable("Ua"), Err(BadName::Message(_))));
        let (m, r1, r2, _) = parsematname("US:1-3", &s);
        assert_eq!((m, r1, r2), (Ok(Some(MatName::U(19))), 1, 3));
        let (m, r1, r2, _) = parsematname("UF:2", &s);
        assert_eq!((m, r1, r2), (Ok(Some(MatName::U(6))), 2, 2));
        let (_, r1, r2, _) = parsematname("U", &s);
        assert_eq!((r1, r2), (1, 999999));
    }
}
