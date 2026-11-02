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
clear UA\x20
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
optimise 12 UA:1  0.0   0.0   0.0   0.0   0.0   0.0   0.0  rel 4\x20

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
        if let Some((i, _)) = r.char_indices().rev().find(|&(_, c)| !blank(c))
            && i > 0
        {
            let end = i + r[i..].chars().next().map_or(1, char::len_utf8);
            r = &r[..end];
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

/// `sort`'s order (§7.1): libstdc++'s `std::sort` as its documentation describes it, an
/// introsort (median-of-three pivots moved to the front, an unguarded partition, the larger
/// part looped and the right one recursed into while more than 16 rows remain, a heap sort when
/// the depth `2·⌊log₂ n⌋` runs out), then an insertion sort (guarded over the first 16 rows,
/// unguarded after them). Rows of equal cost keep the order this gives them (not stable).
pub fn std_sort<T: Clone>(v: &mut [T], less: &impl Fn(&T, &T) -> bool) {
    std_sort_checked(v, less);
}

/// [`std_sort`]; returns `false` where libstdc++'s unguarded scans would have read outside the
/// slice (an inconsistent order, such as `<` with NaN values: the scans stop at its ends here).
pub fn std_sort_checked<T: Clone>(v: &mut [T], less: &impl Fn(&T, &T) -> bool) -> bool {
    let n = v.len();
    if n < 2 {
        return true;
    }
    let inside = std::cell::Cell::new(true);
    let less = |a: &T, b: &T| less(a, b);
    let guard = Guard { inside: &inside };
    let depth = 2 * (usize::BITS - 1 - n.leading_zeros()) as usize;
    introsort_loop(v, 0, n, depth, &less, &guard);
    if n > 16 {
        insertion_sort(v, 0, 16, &less, &guard);
        for i in 16..n {
            unguarded_linear_insert(v, i, &less, &guard);
        }
    } else {
        insertion_sort(v, 0, n, &less, &guard);
    }
    inside.get()
}

/// Notes a scan that would have left the slice.
struct Guard<'a> {
    inside: &'a std::cell::Cell<bool>,
}

impl Guard<'_> {
    fn out(&self) {
        self.inside.set(false);
    }
}

fn introsort_loop<T: Clone>(
    v: &mut [T],
    first: usize,
    mut last: usize,
    mut depth: usize,
    less: &impl Fn(&T, &T) -> bool,
    guard: &Guard<'_>,
) {
    while last - first > 16 {
        if depth == 0 {
            heap_sort(&mut v[first..last], less);
            return;
        }
        depth -= 1;
        let mid = first + (last - first) / 2;
        move_median_to_first(v, first, first + 1, mid, last - 1, less);
        let cut = unguarded_partition(v, first + 1, last, first, less, guard);
        introsort_loop(v, cut, last, depth, less, guard);
        last = cut;
    }
}

fn move_median_to_first<T>(
    v: &mut [T],
    result: usize,
    a: usize,
    b: usize,
    c: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    let pick = if less(&v[a], &v[b]) {
        if less(&v[b], &v[c]) {
            b
        } else if less(&v[a], &v[c]) {
            c
        } else {
            a
        }
    } else if less(&v[a], &v[c]) {
        a
    } else if less(&v[b], &v[c]) {
        c
    } else {
        b
    };
    v.swap(result, pick);
}

fn unguarded_partition<T>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    pivot: usize,
    less: &impl Fn(&T, &T) -> bool,
    guard: &Guard<'_>,
) -> usize {
    loop {
        while first < v.len() && less(&v[first], &v[pivot]) {
            first += 1;
        }
        if first == v.len() {
            guard.out();
            return first.min(last);
        }
        last -= 1;
        while last > 0 && less(&v[pivot], &v[last]) {
            last -= 1;
        }
        if last == 0 && less(&v[pivot], &v[0]) {
            guard.out();
        }
        if first >= last {
            return first;
        }
        v.swap(first, last);
        first += 1;
    }
}

fn insertion_sort<T: Clone>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
    guard: &Guard<'_>,
) {
    for i in first + 1..last {
        if less(&v[i], &v[first]) {
            let x = v[i].clone();
            for j in (first..i).rev() {
                v[j + 1] = v[j].clone();
            }
            v[first] = x;
        } else {
            unguarded_linear_insert(v, i, less, guard);
        }
    }
}

fn unguarded_linear_insert<T: Clone>(
    v: &mut [T],
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
    guard: &Guard<'_>,
) {
    let x = v[last].clone();
    let mut hole = last;
    while hole > 0 && less(&x, &v[hole - 1]) {
        v[hole] = v[hole - 1].clone();
        hole -= 1;
    }
    if hole == 0 && last > 0 {
        // libstdc++ would compare with the value before the slice next
        guard.out();
    }
    v[hole] = x;
}

/// `std::partial_sort` over the whole range: the heap made, then popped from the back.
fn heap_sort<T: Clone>(v: &mut [T], less: &impl Fn(&T, &T) -> bool) {
    let len = v.len();
    if len < 2 {
        return;
    }
    let mut parent = (len - 2) / 2;
    loop {
        let x = v[parent].clone();
        adjust_heap(v, parent, len, x, less);
        if parent == 0 {
            break;
        }
        parent -= 1;
    }
    let mut end = len;
    while end > 1 {
        end -= 1;
        let x = v[end].clone();
        v[end] = v[0].clone();
        adjust_heap(v, 0, end, x, less);
    }
}

fn adjust_heap<T: Clone>(
    v: &mut [T],
    mut hole: usize,
    len: usize,
    x: T,
    less: &impl Fn(&T, &T) -> bool,
) {
    let top = hole;
    let mut second = hole;
    while second < (len - 1) / 2 {
        second = 2 * (second + 1);
        if less(&v[second], &v[second - 1]) {
            second -= 1;
        }
        v[hole] = v[second].clone();
        hole = second;
    }
    if len % 2 == 0 && second == (len - 2) / 2 {
        second = 2 * (second + 1);
        v[hole] = v[second - 1].clone();
        hole = second - 1;
    }
    let mut parent = hole.wrapping_sub(1) / 2;
    while hole > top && less(&v[parent], &x) {
        v[hole] = v[parent].clone();
        hole = parent;
        parent = hole.wrapping_sub(1) / 2;
    }
    v[hole] = x;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_sort_sorts() {
        let mut state = 12345u64;
        for n in [0usize, 1, 2, 5, 16, 17, 40, 300, 1000] {
            let mut v: Vec<(u32, usize)> = (0..n)
                .map(|i| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (((state >> 33) % 7) as u32, i)
                })
                .collect();
            std_sort(&mut v, &|a, b| a.0 < b.0);
            assert!(v.windows(2).all(|w| w[0].0 <= w[1].0), "{n}");
            let mut ids: Vec<usize> = v.iter().map(|x| x.1).collect();
            ids.sort_unstable();
            assert_eq!(ids, (0..n).collect::<Vec<_>>());
        }
    }

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
