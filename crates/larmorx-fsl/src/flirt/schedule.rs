// SPDX-License-Identifier: Apache-2.0
//! FLIRT's schedule language (`specs/flirt.md` §7.1): lines split into words, the matrix
//! variables and their row ranges, the scalars, and the default schedules (§7.4), whose lines
//! FLIRT prints with `-v` (their text as FSL 6.0.7.23 prints it, observed).

/// The 3D default schedule (§7.4), line by line as `flirt -v` prints it (observed).
pub const DEFAULT_3D: &str = "# 8mm scale
setscale 8
setoption smoothing 8
clear S
clear P
search
# 4mm scale
setscale 4
setoption smoothing 4
clear U
clear UA
clear UB
clear US
clear UP
# remeasure costs at this scale
measurecost 7 S 0 0 0 0 0 0 rel
copy U US
clear U
measurecost 7 P 0 0 0 0 0 0 rel
copy U UP
dualsort US UP
# optimise best 3 candidates (pre and post 8mm optimisations)
clear U
optimise 7 US:1-3  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4
optimise 7 UP:1-3  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4
# also try the identity transform as a starting point at this resolution
clear UQ
setrow UQ  1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
optimise 7 UQ  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4
sort U
copy U UA
# select best 4 optimised solutions and try perturbations of these
clear U
copy UA:1-4 U
optimise 7 UA:1-4  1.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4
optimise 7 UA:1-4 -1.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4
optimise 7 UA:1-4  0.0   1.0   0.0   0.0   0.0   0.0   0.0  rel 4
optimise 7 UA:1-4  0.0  -1.0   0.0   0.0   0.0   0.0   0.0  rel 4
optimise 7 UA:1-4  0.0   0.0   1.0   0.0   0.0   0.0   0.0  rel 4
optimise 7 UA:1-4  0.0   0.0  -1.0   0.0   0.0   0.0   0.0  rel 4
optimise 7 UA:1-4  0.0   0.0   0.0   0.0   0.0   0.0   0.1  abs 4
optimise 7 UA:1-4  0.0   0.0   0.0   0.0   0.0   0.0  -0.1  abs 4
optimise 7 UA:1-4  0.0   0.0   0.0   0.0   0.0   0.0   0.2  abs 4
optimise 7 UA:1-4  0.0   0.0   0.0   0.0   0.0   0.0  -0.2  abs 4
sort U
copy U UB
# 2mm scale
setscale 2
setoption smoothing 2
clear U
clear UC
clear UD
clear UE
clear UF
# remeasure costs at this scale
measurecost 7 UB 0 0 0 0 0 0 rel
sort U
copy U UC
clear U
optimise 7  UC:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4
copy U UD
setoption boundguess 1
if MAXDOF > 7
 clear U
if MAXDOF > 7
 optimise 9  UD:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1
copy U UE
if MAXDOF > 9
 clear U
if MAXDOF > 9
 optimise 12 UE:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 2
sort U
copy U UF
# 1mm scale
setscale 1
setoption smoothing 1
setoption boundguess 1
clear U
# also try the qsform as a starting point at this resolution
setrowqsform UF
optimise 12 UF:1-2  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1
# in addition, try qsform as the final transformation, not just an initialisation
clear UG
setrowqsform UG
measurecost 12 UG:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1
sort U";

/// The 2D default schedule (`-2D`, §7.4), as `flirt -2D -v` prints it (observed).
pub const DEFAULT_2D: &str = "# 8mm scale
setscale 8
setoption smoothing 8
setoption paramsubset 3  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0
clear U
clear UA
setrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
optimise 12 UA:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4

# 4mm scale
setscale 4
setoption smoothing 4
setoption paramsubset 3  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0
clear UB
clear UL
clear UM
# remeasure costs at this scale
clear U
measurecost 12 UA 0 0 0 0 0 0 rel
sort U
copy U UL
# optimise best 3 candidates
clear U
optimise 12 UL:1-3  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4
# also try the identity transform as a starting point at this resolution
clear UQ
setrow UQ  1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
optimise 7 UQ  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4
sort U
copy U UM
# select best 4 optimised solutions and try perturbations of these
clear U
copy UM:1-4 U
optimise 12 UM:1-4  0.0   0.0   1.0   0.0   0.0   0.0   0.0  rel 4
optimise 12 UM:1-4  0.0   0.0  -1.0   0.0   0.0   0.0   0.0  rel 4
sort U
clear UB
copy U UB

# 2mm scale
setscale 2
setoption smoothing 2
setoption paramsubset 3  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0
clear U
clear UC
clear UD
clear UE
clear UF
# remeasure costs at this scale
measurecost 12 UB 0 0 0 0 0 0 rel
sort U
copy U UC
clear U
optimise 12  UC:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4
copy U UD
setoption boundguess 1
if MAXDOF > 7
 clear U
if MAXDOF > 7
 optimise 9  UD:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1
copy U UE
if MAXDOF > 9
 clear U
if MAXDOF > 9
 optimise 12 UE:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 2
sort U
copy U UF

# 1mm scale
setscale 1
setoption smoothing 1
setoption boundguess 1
setoption paramsubset 3  0 0 1 0 0 0 0 0 0 0 0 0  0 0 0 1 0 0 0 0 0 0 0 0  0 0 0 0 1 0 0 0 0 0 0 0
clear U
# also try the qsform as a starting point at this resolution
setrowqsform UF
optimise 12 UF:1-2  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 1
sort U";

fn blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// The words of a schedule line (§7.1): leading blanks and tabs removed, trailing ones removed
/// unless the last non-blank character is the remainder's first, the first word up to the next
/// blank or tab, repeated on the rest.
pub fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut r: &str = line.trim_end_matches(['\r', '\n']);
    if r.trim_matches(blank).is_empty() {
        return out;
    }
    while !r.is_empty() {
        r = r.trim_start_matches(blank);
        if let Some((i, _)) = r.char_indices().rev().find(|&(_, c)| !blank(c)) {
            if i > 0 {
                let end = i + r[i..].chars().next().map_or(1, char::len_utf8);
                r = &r[..end];
            }
        }
        match r.find(blank) {
            Some(j) => {
                out.push(r[..j].to_owned());
                r = &r[j + 1..];
            }
            None => {
                out.push(r.to_owned());
                r = "";
            }
        }
    }
    out
}

/// A matrix variable (§7.1): `S`, `P`, `U` (also `U` and a digit) or `UA` … `UZ`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Var {
    S,
    P,
    /// `U` (index 0) and `UA` … `UZ` (1 … 26).
    U(u8),
}

/// A matrix name that is not one (§7.1): FSL's message (FSL reads an uninitialised list there;
/// larmorx refuses).
pub fn parse_var(name: &str) -> Result<Var, String> {
    let b = name.as_bytes();
    match b {
        [b'S'] => Ok(Var::S),
        [b'P'] => Ok(Var::P),
        [b'U'] => Ok(Var::U(0)),
        [b'U', c, ..] if c.is_ascii_digit() => Ok(Var::U(0)),
        [b'U', c, ..] if c.is_ascii_uppercase() => Ok(Var::U(c - b'A' + 1)),
        [b'U', ..] => Err(format!("Can only accept UA to UZ, not {name}")),
        _ => Err(format!("Cannot interpret {name} as a Matrix name")),
    }
}

/// The value of a scalar word (§7.1): `MAXDOF`, `MINSAMPLING`, 0 for another word starting with
/// a letter, else C's `atof`.
pub fn scalar(word: &str, maxdof: f64, minsampling: f64) -> f64 {
    match word {
        "MAXDOF" => maxdof,
        "MINSAMPLING" => minsampling,
        w if w.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) => 0.0,
        w => super::mat::atof(w),
    }
}

/// An integer scalar: rounded half away from zero (`(int)(x ± 0.5)` in double).
pub fn int_scalar(v: f64) -> i64 {
    if v >= 0.0 {
        (v + 0.5) as i64
    } else {
        (v - 0.5) as i64
    }
}

/// A matrix argument `NAME[:r1-r2]` (§7.1): the variable and the 1-based row range as given
/// (default `1-999999`), not yet clipped.
pub fn parse_ref(word: &str, maxdof: f64, minsampling: f64) -> Result<(Var, i64, i64), String> {
    let (name, rows) = match word.split_once(':') {
        Some((n, r)) => (n, Some(r)),
        None => (word, None),
    };
    let var = parse_var(name)?;
    let (r1, r2) = match rows {
        None => (1, 999_999),
        Some(r) => match r.split_once('-') {
            Some((a, b)) => (
                int_scalar(scalar(a, maxdof, minsampling)),
                int_scalar(scalar(b, maxdof, minsampling)),
            ),
            None => {
                let v = int_scalar(scalar(r, maxdof, minsampling));
                (v, v)
            }
        },
    };
    let r1 = r1.max(1);
    let r2 = r2.max(r1);
    Ok((var, r1, r2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitting() {
        assert_eq!(words("clear U"), ["clear", "U"]);
        assert_eq!(words("  setrow UQ  1 0"), ["setrow", "UQ", "1", "0"]);
        assert_eq!(words("clear U  "), ["clear", "U"]);
        assert!(words("").is_empty());
        assert!(words("   ").is_empty());
        assert_eq!(words("# 8mm scale")[0], "#");
    }

    #[test]
    fn names() {
        assert_eq!(parse_var("U").unwrap(), Var::U(0));
        assert_eq!(parse_var("U3").unwrap(), Var::U(0));
        assert_eq!(parse_var("UZ").unwrap(), Var::U(26));
        assert!(parse_var("Uq").is_err());
        assert!(parse_var("X").is_err());
        assert_eq!(parse_ref("UA:1-4", 12.0, 1.0).unwrap(), (Var::U(1), 1, 4));
        assert_eq!(parse_ref("UA:2", 12.0, 1.0).unwrap(), (Var::U(1), 2, 2));
        assert_eq!(parse_ref("S", 12.0, 1.0).unwrap(), (Var::S, 1, 999_999));
    }
}
