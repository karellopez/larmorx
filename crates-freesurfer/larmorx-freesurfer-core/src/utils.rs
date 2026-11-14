// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's small helpers (`fs.core`, queue item 3.1-22), translated from FreeSurfer 8.2.0's
//! `utils/utils.cpp` (its random numbers are [`crate::random`]): reading lines, strings and file
//! names, IEEE tests, angles, rounding, lists of integers, order statistics, the fast arctangent,
//! memory reports, the logic-problem reports and the temporary files.
//!
//! FreeSurfer's strings are C strings (bytes, ended by the first NUL); the C locale's
//! classification applies throughout (FreeSurfer sets only `LC_NUMERIC`), so `isspace` is
//! space, `\t`, `\n`, `\v`, `\f`, `\r` and `toupper` changes ASCII letters only.
//!
//! - **FS-06** (`fast_approx_atan2f`): the table index `(100000 * ay) / ax` in `float` is NaN,
//!   or infinite, for a NaN argument, two infinite ones, or a larger magnitude so large that the
//!   product overflows; its conversion to `int` then gives `INT_MIN` and the lookup reads 8 GB
//!   before the table. Both modes give the value without the lookup's overflow (the product
//!   computed on both magnitudes scaled by 2^-64, exact, so the quotient is the one an unbounded
//!   `float` exponent gives) or, for NaN and infinite pairs, `atan2f`'s, with a notice.
//! - **FS-09** (`median`, `quick_select`): an empty list reads the value before it; both modes
//!   give NaN with a notice.
//! - **FS-10** (the unique-integer helpers): an empty list reads the integer past it and reports
//!   one item. The default reports none; `reproduce` reports one, its value (memory FreeSurfer
//!   never wrote) filled with 0, with a notice.
//! - **FS-11** (`stricmp`, `file_name_extension`): an argument of 1,024 bytes or more overflows a
//!   stack buffer; both modes work on the whole string, with a notice.
//! - **FS-12** (`nint`, `nint2`): out-of-range values and NaN; the default saturates (NaN gives
//!   0), `reproduce` gives `INT_MIN` (x86-64's result), with a notice.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use larmorx_core::defects::{FS_06, FS_09, FS_10, FS_11, FS_12};

use crate::cstd;
use crate::diag::{self, BUILD_ROOT};
use crate::globals::Globals;
use crate::log::{self, Fatal};
use crate::run::Environment;
use crate::sse;

/// `STR_LEN` (`include/const.h`): FreeSurfer's usual string buffer.
pub const STR_LEN: usize = 1024;

fn c_str(s: &[u8]) -> &[u8] {
    match s.iter().position(|&b| b == 0) {
        Some(n) => &s[..n],
        None => s,
    }
}

// ---- lines, strings and file names --------------------------------------------------------

/// `fgetl(s, n, fp)`: the next line (read as `fgets` with a buffer of `n` bytes: at most `n - 1`
/// bytes, through the newline) that is not empty, blank or a comment, without its leading white
/// space, everything from its first `#` on, and its newline; `None` at the end of the file.
pub fn fgetl(reader: &mut dyn BufRead, n: usize) -> Option<Vec<u8>> {
    loop {
        let line = fgets(reader, n)?;
        let line = c_str(&line);
        let start = line
            .iter()
            .position(|&b| !cstd::is_space(b))
            .unwrap_or(line.len());
        let cp = &line[start..];
        if cp.is_empty() || cp[0] == b'#' || cp[0] == b'\n' {
            continue;
        }
        let end = cp.iter().position(|&b| b == b'#').unwrap_or(cp.len());
        let mut cp = cp[..end].to_vec();
        if cp.last() == Some(&b'\n') {
            cp.pop();
        }
        return Some(cp);
    }
}

/// `fgets(s, n, fp)`: at most `n - 1` bytes, through a newline; `None` when nothing is left (or
/// `n` < 2, where glibc reads nothing).
pub fn fgets(reader: &mut dyn BufRead, n: usize) -> Option<Vec<u8>> {
    if n < 2 {
        return None;
    }
    let mut line = Vec::new();
    while line.len() < n - 1 {
        let buf = reader.fill_buf().ok()?;
        if buf.is_empty() {
            break;
        }
        let room = n - 1 - line.len();
        let take = match buf.iter().take(room).position(|&b| b == b'\n') {
            Some(i) => i + 1,
            None => buf.len().min(room),
        };
        line.extend_from_slice(&buf[..take]);
        reader.consume(take);
        if line.last() == Some(&b'\n') {
            break;
        }
    }
    if line.is_empty() { None } else { Some(line) }
}

/// `StrUpper` (`str_toupper` too): ASCII letters to upper case (the C locale's `toupper`).
pub fn str_upper(s: &str) -> String {
    s.to_ascii_uppercase()
}

/// `StrLower`.
pub fn str_lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// Truncates `s` at the last `c`.
fn cut_at_last(s: &mut String, c: char) {
    if let Some(i) = s.rfind(c) {
        s.truncate(i);
    }
}

/// `FileNameOnly(full_name, fname)`: the part after the last `/`, cut at its last `#`, then at
/// its last `@`.
pub fn file_name_only(full_name: &str) -> String {
    let mut f = match full_name.rfind('/') {
        Some(i) => full_name[i + 1..].to_owned(),
        None => full_name.to_owned(),
    };
    cut_at_last(&mut f, '#');
    cut_at_last(&mut f, '@');
    f
}

/// `FileName(full_name)`: as [`file_name_only`], after skipping one leading `@`.
pub fn file_name(full_name: &str) -> String {
    let f = match full_name.rfind('/') {
        Some(i) => &full_name[i + 1..],
        None => full_name,
    };
    let mut f = f.strip_prefix('@').unwrap_or(f).to_owned();
    cut_at_last(&mut f, '#');
    cut_at_last(&mut f, '@');
    f
}

/// `FileExists(fname)`: whether `fopen(fname, "r")` succeeds (a directory does, on Linux), with
/// `errno` left as it was. Relative names start in `dir`.
pub fn file_exists(dir: Option<&Path>, fname: &str) -> bool {
    let p = resolve(dir, fname);
    #[cfg(unix)]
    {
        std::fs::File::open(&p).is_ok()
    }
    #[cfg(not(unix))]
    {
        p.is_dir() || std::fs::File::open(&p).is_ok()
    }
}

/// A file name as a program in `dir` reads it.
pub fn resolve(dir: Option<&Path>, fname: &str) -> PathBuf {
    match dir {
        // an empty name stays empty: C's open("") fails (ENOENT), where the folder would open
        Some(d) if !fname.is_empty() && !Path::new(fname).is_absolute() => d.join(fname),
        _ => PathBuf::from(fname),
    }
}

/// glibc's `strcmp` result: the difference of the first differing bytes (as unsigned), 0 when
/// equal.
pub fn strcmp(a: &[u8], b: &[u8]) -> i32 {
    let (a, b) = (c_str(a), c_str(b));
    for i in 0.. {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y || x == 0 {
            return i32::from(x) - i32::from(y);
        }
    }
    0
}

/// FS-11's check: a string of `STR_LEN` bytes or more overflows FreeSurfer's buffer.
fn fs11(g: &mut Globals, what: &str, s: &str) {
    if s.len() >= STR_LEN {
        g.notices.add(
            &FS_11,
            format!(
                "{what} of a {}-byte string, longer than FreeSurfer's buffer",
                s.len()
            ),
        );
    }
}

/// `stricmp(str1, str2)`: `strcmp` of the two in upper case (FS-11).
pub fn stricmp(g: &mut Globals, str1: &str, str2: &str) -> i32 {
    fs11(g, "stricmp", str1);
    fs11(g, "stricmp", str2);
    strcmp(str_upper(str1).as_bytes(), str_upper(str2).as_bytes())
}

/// `FileNamePath(fname, pathName)`: everything before the last `/` (empty for `/name`), or `.`
/// without one (the Linux build's choice).
pub fn file_name_path(fname: &str) -> String {
    match fname.rfind('/') {
        Some(i) => fname[..i].to_owned(),
        None => ".".to_owned(),
    }
}

/// `FileNameRemoveExtension`: cut at the last `.`, wherever it is.
pub fn file_name_remove_extension(fname: &str) -> String {
    let mut f = fname.to_owned();
    cut_at_last(&mut f, '.');
    f
}

/// `FileNameExtension(fname, ext)`: after the last `.`, or empty (FS-11).
pub fn file_name_extension(g: &mut Globals, fname: &str) -> String {
    fs11(g, "FileNameExtension", fname);
    match fname.rfind('.') {
        Some(i) => fname[i + 1..].to_owned(),
        None => String::new(),
    }
}

/// `FileNameFromWildcard(inStr, outStr)`: a name with a `*` is replaced by the first path
/// `glob(inStr, 0)` finds, when it finds any; any other name is returned as it is.
pub fn file_name_from_wildcard(dir: Option<&Path>, in_str: &str) -> String {
    if in_str.contains('*')
        && let Some(first) = crate::cglob::glob(dir, in_str).into_iter().next()
    {
        return first;
    }
    in_str.to_owned()
}

/// `ItemsInString(str)`: the runs of non-space bytes; -1 for an empty string.
pub fn items_in_string(s: &[u8]) -> i32 {
    let s = c_str(s);
    if s.is_empty() {
        return -1;
    }
    s.split(|&b| cstd::is_space(b))
        .filter(|w| !w.is_empty())
        .count() as i32
}

/// `deblank(str)`: the string without its space bytes.
pub fn deblank(s: &[u8]) -> Vec<u8> {
    c_str(s)
        .iter()
        .copied()
        .filter(|&b| !cstd::is_space(b))
        .collect()
}

/// `CountItemsInString(str)`: the runs of bytes other than space and tab, stopping where an
/// item would start with `\n` or `\r`.
pub fn count_items_in_string(s: &[u8]) -> i32 {
    let s = c_str(s);
    let blank = |b: u8| b == b' ' || b == b'\t';
    let (mut n, mut hits) = (0, 0);
    while n < s.len() {
        while n < s.len() && blank(s[n]) {
            n += 1;
        }
        if n >= s.len() || s[n] == b'\n' || s[n] == b'\r' {
            break;
        }
        while n < s.len() && !blank(s[n]) {
            n += 1;
        }
        hits += 1;
    }
    hits
}

/// `GetNthItemFromString(str, nth)`: the `nth` white-space separated item (`-1`, or any
/// negative `nth`, the last); otherwise the error `asking for item <nth>, but <n> items in
/// string` and `None`.
pub fn get_nth_item_from_string(
    g: &Globals,
    err: &mut dyn Write,
    s: &[u8],
    nth: i32,
) -> Option<Vec<u8>> {
    let tokens: Vec<&[u8]> = c_str(s)
        .split(|&b| cstd::is_space(b))
        .filter(|w| !w.is_empty())
        .collect();
    let nth = if nth < 0 {
        tokens.len() as i64 - 1
    } else {
        i64::from(nth)
    };
    match usize::try_from(nth).ok().and_then(|i| tokens.get(i)) {
        Some(t) => Some(t.to_vec()),
        None => {
            log::error(
                g,
                err,
                &format!(
                    "asking for item {}, but {} items in string",
                    nth as i32,
                    tokens.len()
                ),
            );
            None
        }
    }
}

/// `FindFirstMatch(iv, i)`: the first index of `i`, or -1.
pub fn find_first_match(iv: &[i32], i: i32) -> i32 {
    iv.iter().position(|&x| x == i).map_or(-1, |p| p as i32)
}

// ---- numbers -----------------------------------------------------------------------------

/// `devIsinf(value)`: 1 for +inf, -1 for -inf, 0 otherwise (from the bits).
pub fn dev_isinf(value: f32) -> i32 {
    match value.to_bits() {
        0x7f80_0000 => 1,
        0xff80_0000 => -1,
        _ => 0,
    }
}

/// `devIsnan(value)`: 1 for a NaN (exponent all ones, fraction not 0), else 0.
pub fn dev_isnan(value: f32) -> i32 {
    i32::from(value.is_nan())
}

/// `devFinite(value)`.
pub fn dev_finite(value: f32) -> i32 {
    i32::from(dev_isinf(value) == 0 && dev_isnan(value) == 0)
}

/// `deltaAngle(angle1, angle2)`: `angle1 - angle2` in `float`, folded once by 2π when it is
/// beyond ±π (compared in `double`).
pub fn delta_angle(angle1: f32, angle2: f32) -> f32 {
    let delta = angle1 - angle2;
    let pi = std::f64::consts::PI;
    if f64::from(delta) > pi {
        2.0f32 * (pi as f32) - delta
    } else if f64::from(delta) < -pi {
        -2.0f32 * (pi as f32) - delta
    } else {
        delta
    }
}

/// The conversion `(int) x` of `nint` and `nint2` (FS-12).
fn to_int(g: &mut Globals, x: f64, what: &str) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        g.notices.add(
            &FS_12,
            format!("{what} of {x}, outside the int range: an undefined conversion"),
        );
        if g.notices.reproduce() {
            return i32::MIN;
        }
        if x.is_nan() {
            return 0;
        }
        return if x > 0.0 { i32::MAX } else { i32::MIN };
    }
    x.trunc() as i32
}

/// `DListStats(dlist, nlist, stats)`: `[nlist, mean, stddev, min, max]` of the list: the sum and
/// the sum of the `double` squares in x87 `long double` (`larmorx_core::x87::F80`), `mean = sum /
/// nlist` (as a `double`), `stddev = sqrtl(sum2 / nlist - mean * mean)` (the square of the
/// `double` mean in `double`, the rest in `long double`), as a `double`; min and max from the
/// first element. `None` after `ERROR: DListStats() nlist = <n>` (or `... dlist is NULL`) on
/// standard output.
pub fn dlist_stats(out: &mut dyn Write, dlist: Option<&[f64]>, nlist: i32) -> Option<[f64; 5]> {
    use larmorx_core::x87::F80;
    if nlist <= 0 {
        let _ = writeln!(out, "ERROR: DListStats() nlist = {nlist}");
        return None;
    }
    let Some(dlist) = dlist else {
        let _ = writeln!(out, "ERROR: DListStats() dlist is NULL");
        return None;
    };
    let list = &dlist[..nlist as usize];
    let (mut sum, mut sum2) = (F80::ZERO, F80::ZERO);
    let (mut min, mut max) = (list[0], list[0]);
    for &x in list {
        sum += F80::from_f64(x);
        sum2 += F80::from_f64(crate::sse::mul64(x, x));
        if min > x {
            min = x;
        }
        if max < x {
            max = x;
        }
    }
    let n = F80::from(nlist);
    let mean = (sum / n).to_f64();
    let stddev = (sum2 / n - F80::from_f64(crate::sse::mul64(mean, mean)))
        .sqrt()
        .to_f64();
    Some([f64::from(nlist), mean, stddev, min, max])
}

/// `nint(f)`: `f + 0.5` (or `f - 0.5` below 0) truncated (FS-12).
pub fn nint(g: &mut Globals, f: f64) -> i32 {
    to_int(g, if f < 0.0 { f - 0.5 } else { f + 0.5 }, "nint")
}

/// `nint2(f)`: as [`nint`], with `f + 0.49999999` at or above 0.
pub fn nint2(g: &mut Globals, f: f64) -> i32 {
    to_int(g, if f < 0.0 { f - 0.5 } else { f + 0.49999999 }, "nint2")
}

/// `sum2stddev(xsum, xsum2, nx)`: `sqrt((xsum2 - 2*mean*xsum + nx*mean*mean) / (nx - 1))`, mean
/// `xsum / nx`, in that order.
pub fn sum2stddev(xsum: f64, xsum2: f64, nx: i32) -> f64 {
    use crate::sse::{add64, div64, mul64, sqrt64, sub64};
    let n = f64::from(nx);
    let xmean = div64(xsum, n);
    let t = add64(
        sub64(xsum2, mul64(mul64(2.0, xmean), xsum)),
        mul64(mul64(n, xmean), xmean),
    );
    // SSE2's NaN rules (crate::sse): nx of 0 or 1 makes 0 / 0
    sqrt64(div64(t, f64::from(nx.wrapping_sub(1))))
}

/// `compare_ints`: -1, 1 or 0.
pub fn compare_ints(i1: i32, i2: i32) -> i32 {
    match i1.cmp(&i2) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Equal => 0,
    }
}

/// `compare_floats`: -1, 1, or 0 (equal, or a NaN either side).
pub fn compare_floats(i1: f32, i2: f32) -> i32 {
    if i1 < i2 {
        -1
    } else if i1 > i2 {
        1
    } else {
        0
    }
}

/// FS-10 for an empty list.
fn fs10(g: &mut Globals, what: &str) -> bool {
    g.notices.add(
        &FS_10,
        format!("{what} of an empty list: FreeSurfer reads past it and reports one item"),
    );
    g.notices.reproduce()
}

/// `nunqiue_int_list(idlist, nlist)`: sorts the list and counts its distinct values (FS-10).
pub fn nunique_int_list(g: &mut Globals, idlist: &mut [i32]) -> i32 {
    if idlist.is_empty() {
        return i32::from(fs10(g, "nunqiue_int_list"));
    }
    idlist.sort_unstable();
    1 + idlist.windows(2).filter(|w| w[0] != w[1]).count() as i32
}

/// `unqiue_int_list(idlist, nlist, &nunique)`: sorts the list; its distinct values (FS-10).
pub fn unique_int_list(g: &mut Globals, idlist: &mut [i32]) -> Vec<i32> {
    if idlist.is_empty() {
        return if fs10(g, "unqiue_int_list") {
            vec![0]
        } else {
            vec![]
        };
    }
    idlist.sort_unstable();
    let mut u = idlist.to_vec();
    u.dedup();
    u
}

/// `most_frequent_int_list(idlist, nlist, &nmax)`: sorts the list; its most frequent value (the
/// smallest of a tie) and its count (FS-10: an empty list gives `(0, 0)`).
pub fn most_frequent_int_list(g: &mut Globals, idlist: &mut [i32]) -> (i32, i32) {
    if idlist.is_empty() {
        fs10(g, "most_frequent_int_list");
        return (0, 0);
    }
    let ulist = unique_int_list(g, idlist);
    let (mut best, mut nmax) = (0usize, -1i32);
    for (k, &u) in ulist.iter().enumerate() {
        let n = idlist.iter().filter(|&&x| x == u).count() as i32;
        if nmax < n {
            nmax = n;
            best = k;
        }
    }
    (ulist[best], nmax)
}

/// `kth_smallest(a, n, k)`: Wirth's selection of the `k`-th smallest (from 1), reordering `a`.
pub fn kth_smallest(a: &mut [f32], k: usize) -> f32 {
    let kk = k as isize - 1;
    let (mut l, mut m) = (0isize, a.len() as isize - 1);
    while l < m {
        let x = a[kk as usize];
        let (mut i, mut j) = (l, m);
        loop {
            while a[i as usize] < x {
                i += 1;
            }
            while x < a[j as usize] {
                j -= 1;
            }
            if i <= j {
                a.swap(i as usize, j as usize);
                i += 1;
                j -= 1;
            }
            if i > j {
                break;
            }
        }
        if j < kk {
            l = i;
        }
        if kk < i {
            m = j;
        }
    }
    a[kk as usize]
}

/// `quick_select(arr, n, k)`: Numerical Recipes' selection of the `k`-th smallest (from 1),
/// reordering `arr` (FS-09 for an empty list: NaN).
pub fn quick_select(g: &mut Globals, arr: &mut [f32], k: usize) -> f32 {
    if arr.is_empty() {
        g.notices.add(
            &FS_09,
            "the median of an empty list: NaN, not the value before the list",
        );
        return f32::NAN;
    }
    let (mut low, mut high) = (0usize, arr.len() - 1);
    let median = k - 1;
    loop {
        if high <= low {
            return arr[median];
        }
        if high == low + 1 {
            if arr[low] > arr[high] {
                arr.swap(low, high);
            }
            return arr[median];
        }
        let middle = (low + high) / 2;
        if arr[middle] > arr[high] {
            arr.swap(middle, high);
        }
        if arr[low] > arr[high] {
            arr.swap(low, high);
        }
        if arr[middle] > arr[low] {
            arr.swap(middle, low);
        }
        arr.swap(middle, low + 1);
        let (mut ll, mut hh) = (low + 1, high);
        loop {
            // do ll++ while (arr[low] > arr[ll]); do hh-- while (arr[hh] > arr[low]);
            ll += 1;
            while arr[low] > arr[ll] {
                ll += 1;
            }
            hh -= 1;
            while arr[hh] > arr[low] {
                hh -= 1;
            }
            if hh < ll {
                break;
            }
            arr.swap(ll, hh);
        }
        arr.swap(low, hh);
        if hh <= median {
            low = ll;
        }
        if hh >= median {
            high = hh - 1;
        }
    }
}

/// `median(t, n)`: `kth_smallest` of the middle for an odd count, the mean (in `float`) of two
/// `quick_select`s for an even one; reorders `t` (FS-09).
///
/// The official build computes the mean as `(q2 + q) * 0.5f` in single precision (GCC folds the
/// `double` half, exact for every `float`), the second selection in the destination: of two NaNs
/// the second's is passed on (SSE's rule, `sse::add`), on every platform and in debug and release
/// builds alike.
pub fn median(g: &mut Globals, t: &mut [f32]) -> f32 {
    let n = t.len();
    if n % 2 == 1 {
        return kth_smallest(t, n.div_ceil(2));
    }
    let q = quick_select(g, t, n / 2);
    let q2 = quick_select(g, t, n / 2 + 1);
    sse::mul(sse::add(q2, q), 0.5)
}

// ---- the fast arctangent -----------------------------------------------------------------

const TABLE_LAST: i32 = 100_000;

/// `fastApproxAtan2fWkr`'s table: `atan2(index, 100000)` in `double`, stored as `float`.
fn atan_table() -> &'static [f32] {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..=TABLE_LAST)
            .map(|i| larmorx_core::math::atan2(f64::from(i), f64::from(TABLE_LAST)) as f32)
            .collect()
    })
}

/// `fastApproxAtan2fWkr(ay, ax)` (`ax >= ay >= 0`): `table[(int)((100000 * ay) / ax)]`, 0 when
/// `ax` is 0; `None` where FreeSurfer's index is not a number (FS-06).
fn atan_wkr(ay: f32, ax: f32) -> Option<f32> {
    if ax == 0.0 {
        return Some(atan_table()[0]);
    }
    let mut q = (TABLE_LAST as f32 * ay) / ax;
    if q.is_infinite() && ax.is_finite() {
        // The product overflowed: both magnitudes scaled by 2^-64 (exact for such values).
        let s = 2f32.powi(-64);
        q = (TABLE_LAST as f32 * (ay * s)) / (ax * s);
    }
    if !q.is_finite() {
        return None;
    }
    Some(atan_table()[q as i32 as usize])
}

/// `fastApproxAtan2f(y, x)`: the table's arctangent, or `atan2f` when the variable
/// `FREESURFER_fastApproxAtan2f_use_atanf2` is set (read at the first call, which then prints
/// `<utils.cpp>:1727 using slow atan2f`). Signs: `x == |x|` counts -0 as positive (FS-06).
pub fn fast_approx_atan2f(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    y: f32,
    x: f32,
) -> f32 {
    let use_atan2f = *g.atan2f_choice.get_or_insert_with(|| {
        let set = env.get("FREESURFER_fastApproxAtan2f_use_atanf2").is_some();
        if set {
            let _ = writeln!(out, "{BUILD_ROOT}/utils/utils.cpp:1727 using slow atan2f");
        }
        set
    });
    if use_atan2f {
        return larmorx_core::math::atan2f_glibc(y, x);
    }
    let pi = std::f64::consts::PI as f32;
    let half_pi = (std::f64::consts::PI / 2.0) as f32;
    let (ax, ay) = (x.abs(), y.abs());
    let wkr = if ax >= ay {
        atan_wkr(ay, ax)
    } else {
        atan_wkr(ax, ay).map(|t| half_pi - t)
    };
    let Some(w) = wkr else {
        g.notices.add(
            &FS_06,
            "the fast arctangent of a NaN or of two infinities: atan2f's value, not a read outside the table",
        );
        return larmorx_core::math::atan2f_glibc(y, x);
    };
    let mut r = f64::from(w);
    if ax == x && ay == y {
    } else if ax == x && ay != y {
        r = -r;
    } else if ax != x && ay == y {
        r = f64::from(pi) - r;
    } else {
        // -Pi + r: the same sum in either order.
        r -= f64::from(pi);
    }
    r as f32
}

// ---- memory ------------------------------------------------------------------------------

fn proc_status() -> Option<String> {
    std::fs::read_to_string("/proc/self/status").ok()
}

/// `getMemoryUsed()`: the process's `VmData` in kB (FreeSurfer runs `grep -i vmdata
/// /proc/<pid>/status | cut -f 2` and reads an integer), or -1 after `getting memoryused failed`
/// (no newline) on standard error when it cannot be read.
pub fn get_memory_used(err: &mut dyn Write) -> i32 {
    let value = proc_status().and_then(|s| {
        s.lines()
            .find(|l| l.to_ascii_lowercase().contains("vmdata"))
            .and_then(|l| l.split('\t').nth(1).map(str::to_owned))
            .and_then(|v| scan_d(v.as_bytes()))
    });
    match value {
        Some(v) => v,
        None => {
            let _ = write!(err, "getting memoryused failed");
            -1
        }
    }
}

/// `scanf("%d")`: an integer after white space (glibc: `strtol`'s value, its low 32 bits), or
/// `None` without one.
pub fn scan_d(s: &[u8]) -> Option<i32> {
    let mut i = 0;
    while i < s.len() && cstd::is_space(s[i]) {
        i += 1;
    }
    let mut j = i;
    if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
        j += 1;
    }
    if j < s.len() && s[j].is_ascii_digit() {
        Some(cstd::atoi(&s[i..]))
    } else {
        None
    }
}

/// `GetMemUsage(u)`: `VmSize`, `VmPeak`, `VmRSS`, `VmData`, `VmStk` in kB from
/// `/proc/self/status` (case-insensitive tags), each left as it was when absent (macOS: all).
pub fn get_mem_usage(u: &mut [i32; 5]) {
    let Some(s) = proc_status() else {
        return;
    };
    for line in s.lines() {
        let mut it = line.split_ascii_whitespace();
        let Some(tag) = it.next() else { continue };
        let k = match tag.to_ascii_lowercase().as_str() {
            "vmsize:" => 0,
            "vmpeak:" => 1,
            "vmrss:" => 2,
            "vmdata:" => 3,
            "vmstk:" => 4,
            _ => continue,
        };
        if let Some(v) = it.next().and_then(|v| scan_d(v.as_bytes())) {
            u[k] = v;
        }
    }
}

/// `GetVmPeak()`: `VmPeak` (its `static` values carried from the previous call).
pub fn get_vm_peak(g: &mut Globals) -> i32 {
    get_mem_usage(&mut g.vm_peak_usage);
    g.vm_peak_usage[1]
}

/// The fields of `struct rusage` that `PrintRUsage` prints.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rusage {
    pub utime: f64,
    pub stime: f64,
    pub maxrss: i64,
    pub ixrss: i64,
    pub idrss: i64,
    pub isrss: i64,
    pub minflt: i64,
    pub majflt: i64,
    pub nswap: i64,
    pub inblock: i64,
    pub oublock: i64,
    pub msgsnd: i64,
    pub msgrcv: i64,
    pub nsignals: i64,
    pub nvcsw: i64,
    pub nivcsw: i64,
}

/// `getrusage(RUSAGE_SELF)` as Linux computes it, from `/proc/self` (times in clock ticks of
/// 1/100 s, `ru_maxrss` from `VmHWM`, the block counts from the bytes read and written in
/// 512-byte units; the fields Linux leaves 0 stay 0); `None` where `/proc` is missing.
pub fn rusage_self() -> Option<Rusage> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let after = &stat[stat.rfind(')')? + 2..];
    let f: Vec<&str> = after.split(' ').collect();
    let num = |i: usize| f.get(i).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    // Fields from the state (field 3 of proc(5)): minflt 10, majflt 12, utime 14, stime 15.
    let mut u = Rusage {
        minflt: num(7),
        majflt: num(9),
        utime: num(11) as f64 / 100.0,
        stime: num(12) as f64 / 100.0,
        ..Rusage::default()
    };
    let status = proc_status().unwrap_or_default();
    let tag = |name: &str| {
        status
            .lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|v| v.split_ascii_whitespace().next()?.parse::<i64>().ok())
            .unwrap_or(0)
    };
    u.maxrss = tag("VmHWM:");
    u.nvcsw = tag("voluntary_ctxt_switches:");
    u.nivcsw = tag("nonvoluntary_ctxt_switches:");
    if let Ok(io) = std::fs::read_to_string("/proc/self/io") {
        let io_tag = |name: &str| {
            io.lines()
                .find_map(|l| l.strip_prefix(name))
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(0)
        };
        u.inblock = io_tag("read_bytes:") >> 9;
        u.oublock = (io_tag("write_bytes:") - io_tag("cancelled_write_bytes:")) >> 9;
    }
    Some(u)
}

/// `PrintRUsage(who, pre, fp)` for `RUSAGE_SELF`: sixteen lines `<pre><name> <value>` (the times
/// `%lf`); returns -1 (getrusage's failure) where the usage cannot be read.
pub fn print_rusage(pre: &str, fp: &mut dyn Write) -> i32 {
    let Some(u) = rusage_self() else {
        return -1;
    };
    let _ = writeln!(fp, "{pre}utimesec    {:.6}", u.utime);
    let _ = writeln!(fp, "{pre}stimesec    {:.6}", u.stime);
    for (name, v) in [
        ("ru_maxrss   ", u.maxrss),
        ("ru_ixrss    ", u.ixrss),
        ("ru_idrss    ", u.idrss),
        ("ru_isrss    ", u.isrss),
        ("ru_minflt   ", u.minflt),
        ("ru_majflt   ", u.majflt),
        ("ru_nswap    ", u.nswap),
        ("ru_inblock  ", u.inblock),
        ("ru_oublock  ", u.oublock),
        ("ru_msgsnd   ", u.msgsnd),
        ("ru_msgrcv   ", u.msgrcv),
        ("ru_nsignals ", u.nsignals),
        ("ru_nvcsw    ", u.nvcsw),
        ("ru_nivcsw   ", u.nivcsw),
    ] {
        let _ = writeln!(fp, "{pre}{name}{v}");
    }
    0
}

/// `WriteRUsage(who, pre, fname)`: [`print_rusage`] into a new file; 1 when it cannot be
/// created.
pub fn write_rusage(dir: Option<&Path>, pre: &str, fname: &str) -> i32 {
    match std::fs::File::create(resolve(dir, fname)) {
        Ok(mut f) => print_rusage(pre, &mut f),
        Err(_) => 1,
    }
}

// ---- logic problems ----------------------------------------------------------------------

/// `LogicProblemResponse`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicProblemResponse {
    Old,
    Fix,
}

/// `ReportEntry`'s counters, by `(file, line)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportEntry {
    check_count: i32,
    check_elide_until: i32,
    report_count: i32,
    report_elide_until: i32,
}

/// The logic-problem reports' table (`reportEntry`'s `static` array) and
/// `copeWithLogicProblem2`'s `static emitReport`.
#[derive(Clone, Debug, Default)]
pub struct LogicReports {
    entries: HashMap<(String, i32), ReportEntry>,
    emit_report: Option<bool>,
}

fn grow(n: i32) -> i32 {
    if n < 32 { n + 1 } else { n.wrapping_mul(2) }
}

/// `reportEntry(file, line)`: the entry, made on first use (`cheapAssert(line < 1000000)`).
fn report_entry<'a>(
    g: &'a mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    file: &str,
    line: i32,
) -> Result<&'a mut ReportEntry, Fatal> {
    if line >= 1_000_000 {
        return Err(diag::assert_failed(
            g,
            out,
            err,
            &format!("{BUILD_ROOT}/utils/utils.cpp"),
            1760,
            "line < 1000000",
        ));
    }
    Ok(g.logic_reports
        .entries
        .entry((file.to_owned(), line))
        .or_insert(ReportEntry {
            check_count: 0,
            check_elide_until: 1,
            report_count: 0,
            report_elide_until: 1,
        }))
}

/// `spendTimeCheckingForLogicProblem(file, line)`: true at each of a place's first 32 calls,
/// then at its 64th, 128th, 256th, ... (the threshold grows by one with the count up to 32, then
/// doubles).
pub fn spend_time_checking_for_logic_problem(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    file: &str,
    line: i32,
) -> Result<bool, Fatal> {
    let e = report_entry(g, out, err, file, line)?;
    e.check_count += 1;
    if e.check_count < e.check_elide_until {
        return Ok(false);
    }
    e.check_elide_until = grow(e.check_elide_until);
    Ok(true)
}

/// `copeWithLogicProblem2(wasReported, envvarFixer, msg, file, line, function)`: `fix` when the
/// variable `envvar_fixer` is set; with `FREESURFER_reportLogicErrors` set (read once), it
/// reports with the same thinning as [`spend_time_checking_for_logic_problem`] (a count of its
/// own) on standard output:
/// `<file>:<line> <function> - probable logic error: <msg>, fixing it` (or `, not fixing it`),
/// then `.  Set <var> to fix it` when a variable is named. Returns the response and whether it
/// reported.
#[allow(clippy::too_many_arguments)]
pub fn cope_with_logic_problem2(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    envvar_fixer: Option<&str>,
    msg: &str,
    file: &str,
    line: i32,
    function: &str,
) -> Result<(LogicProblemResponse, bool), Fatal> {
    let emit = *g
        .logic_reports
        .emit_report
        .get_or_insert_with(|| env.get("FREESURFER_reportLogicErrors").is_some());
    let response = match envvar_fixer {
        Some(v) if env.get(v).is_some() => LogicProblemResponse::Fix,
        _ => LogicProblemResponse::Old,
    };
    if !emit {
        return Ok((response, false));
    }
    let e = report_entry(g, out, err, file, line)?;
    e.report_count += 1;
    if e.report_count < e.report_elide_until {
        return Ok((response, false));
    }
    e.report_elide_until = grow(e.report_elide_until);
    let fixing = if response == LogicProblemResponse::Fix {
        ", fixing it"
    } else {
        ", not fixing it"
    };
    let _ = writeln!(
        out,
        "{file}:{line} {function} - probable logic error: {msg}{fixing}"
    );
    if let Some(v) = envvar_fixer {
        let _ = writeln!(out, ".  Set {v} to fix it");
    }
    Ok((response, true))
}

// ---- the environment and temporary files ----------------------------------------------------

/// `getEnvironVar(key)`: the variable's value, empty when unset.
pub fn get_environ_var(env: &Environment, key: &str) -> String {
    env.get(key)
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `directoryExists(directory)`: `stat` succeeds and names a directory.
pub fn directory_exists(dir: Option<&Path>, directory: &str) -> bool {
    resolve(dir, directory).is_dir()
}

/// The characters `mkstemp` draws its six from.
const TEMP_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Six characters for a temporary name (`XXXXXX`): unpredictable, as glibc's; their choice never
/// reaches a result.
fn temp_letters(attempt: u64) -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(attempt);
    h.write_u32(std::process::id());
    if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        h.write_u128(d.as_nanos());
    }
    let mut v = h.finish();
    (0..6)
        .map(|_| {
            let c = TEMP_CHARS[(v % 62) as usize] as char;
            v /= 62;
            c
        })
        .collect()
}

/// Makes `<base>/tmp.XXXXXX<suffix>` as a new file (`mkstemps`) or directory (`mkdtemp`) with a
/// fresh name, trying as many names as glibc (62^3).
fn make_unique(base: &Path, suffix: &str, directory: bool) -> Option<PathBuf> {
    for attempt in 0..238_328u64 {
        let p = base.join(format!("tmp.{}{suffix}", temp_letters(attempt)));
        let made = if directory {
            std::fs::create_dir(&p).map(|_| ())
        } else {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&p)
                .map(|_| ())
        };
        match made {
            Ok(()) => return Some(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

/// `directoryIsWritable(directory)`: it exists and `mkstemp` can make (and FreeSurfer then
/// removes) a file `tmp.XXXXXX` in it.
pub fn directory_is_writable(dir: Option<&Path>, directory: &str) -> bool {
    if !directory_exists(dir, directory) {
        return false;
    }
    match make_unique(&resolve(dir, directory), "", false) {
        Some(p) => {
            let _ = std::fs::remove_file(p);
            true
        }
        None => false,
    }
}

/// `getBaseTempDir()`: the first of `FS_TMPDIR`, `TMPDIR`, `TMP`, `TEMP`, `TEMPDIR` that is set
/// and writable, else `/tmp` (a set but unwritable last variable is kept, as FreeSurfer's loop
/// leaves it); `fs::fatal` `Can not write to temporary directory <dir>` (exit 1) when the choice
/// is not writable.
pub fn get_base_temp_dir(
    g: &mut Globals,
    env: &Environment,
    dir: Option<&Path>,
    err: &mut dyn Write,
) -> Result<String, Fatal> {
    let mut basedir = String::new();
    for name in ["FS_TMPDIR", "TMPDIR", "TMP", "TEMP", "TEMPDIR"] {
        basedir = get_environ_var(env, name);
        if !basedir.is_empty() && directory_is_writable(dir, &basedir) {
            break;
        }
    }
    if basedir.is_empty() {
        basedir = "/tmp".to_owned();
    }
    if !directory_is_writable(dir, &basedir) {
        log::fatal(
            g,
            err,
            1,
            &format!("Can not write to temporary directory {basedir}"),
        )?;
    }
    Ok(basedir)
}

/// `makeTempFile(suffix)`: a new empty file `<base>/tmp.XXXXXX<suffix>`; `Could not create
/// temporary file in <base>` (exit 1) when it cannot be made.
pub fn make_temp_file(
    g: &mut Globals,
    env: &Environment,
    dir: Option<&Path>,
    err: &mut dyn Write,
    suffix: &str,
) -> Result<String, Fatal> {
    let basedir = get_base_temp_dir(g, env, dir, err)?;
    match make_unique(&resolve(dir, &basedir), suffix, false) {
        Some(p) => Ok(format!(
            "{basedir}/{}",
            p.file_name().unwrap_or_default().to_string_lossy()
        )),
        None => {
            log::fatal(
                g,
                err,
                1,
                &format!("Could not create temporary file in {basedir}"),
            )?;
            Ok(String::new())
        }
    }
}

/// `makeTempDir()`: a new directory `<base>/tmp.XXXXXX`; `Could not create temporary directory
/// in <base>` (exit 1) when it cannot be made.
pub fn make_temp_dir(
    g: &mut Globals,
    env: &Environment,
    dir: Option<&Path>,
    err: &mut dyn Write,
) -> Result<String, Fatal> {
    let basedir = get_base_temp_dir(g, env, dir, err)?;
    match make_unique(&resolve(dir, &basedir), "", true) {
        Some(p) => Ok(format!(
            "{basedir}/{}",
            p.file_name().unwrap_or_default().to_string_lossy()
        )),
        None => {
            log::fatal(
                g,
                err,
                1,
                &format!("Could not create temporary directory in {basedir}"),
            )?;
            Ok(String::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn g(mode: UpstreamDefects) -> Globals {
        Globals::new(mode)
    }

    #[test]
    fn fgetl_skips_comments_blanks_and_strips() {
        let text = b"# head\n\n   \n  a b # c\nlonglineXYZ\n#\nlast";
        let mut r = &text[..];
        assert_eq!(fgetl(&mut r, 100).unwrap(), b"a b ");
        // A buffer of 5 bytes reads 4 at a time: the long line comes in pieces.
        assert_eq!(fgetl(&mut r, 5).unwrap(), b"long");
        assert_eq!(fgetl(&mut r, 5).unwrap(), b"line");
        assert_eq!(fgetl(&mut r, 5).unwrap(), b"XYZ");
        assert_eq!(fgetl(&mut r, 100).unwrap(), b"last");
        assert_eq!(fgetl(&mut r, 100), None);
    }

    #[test]
    fn file_names() {
        assert_eq!(file_name_only("/a/b/c.mgz#3@x"), "c.mgz");
        assert_eq!(file_name_only("x@y#z"), "x");
        assert_eq!(file_name("/a/@b@c#2"), "b");
        assert_eq!(file_name_path("/a/b/c"), "/a/b");
        assert_eq!(file_name_path("/c"), "");
        assert_eq!(file_name_path("c"), ".");
        assert_eq!(file_name_remove_extension("a.b/c"), "a");
        let mut gl = g(UpstreamDefects::Fix);
        assert_eq!(file_name_extension(&mut gl, "a.nii.gz"), "gz");
        assert_eq!(file_name_extension(&mut gl, "abc"), "");
        assert!(gl.notices.is_empty());
        let long = "x".repeat(1024);
        file_name_extension(&mut gl, &long);
        assert!(gl.notices.contains(&FS_11));
    }

    #[test]
    fn strings_and_items() {
        let mut gl = g(UpstreamDefects::Fix);
        assert_eq!(stricmp(&mut gl, "abc", "ABC"), 0);
        assert!(stricmp(&mut gl, "abc", "abd") < 0);
        assert_eq!(strcmp(b"a\xff", b"a\x01"), 254);
        assert_eq!(items_in_string(b""), -1);
        assert_eq!(items_in_string(b"   "), 0);
        assert_eq!(items_in_string(b" a  b\tc\n"), 3);
        // A newline ends the count only where an item would start.
        assert_eq!(count_items_in_string(b" a b\n c"), 3);
        assert_eq!(count_items_in_string(b"a \nb"), 1);
        assert_eq!(count_items_in_string(b"a\n"), 1);
        assert_eq!(count_items_in_string(b"\na"), 0);
        assert_eq!(deblank(b" a b\tc "), b"abc");
        let mut err = Vec::new();
        assert_eq!(
            get_nth_item_from_string(&gl, &mut err, b"x y z", -1).unwrap(),
            b"z"
        );
        assert_eq!(get_nth_item_from_string(&gl, &mut err, b"x", 3), None);
        assert_eq!(get_nth_item_from_string(&gl, &mut err, b"", -1), None);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "error: asking for item 3, but 1 items in string\nerror: asking for item -1, but 0 items in string\n"
        );
        assert_eq!(find_first_match(&[3, 1, 3], 3), 0);
        assert_eq!(find_first_match(&[3, 1, 3], 2), -1);
    }

    #[test]
    fn numbers_and_fs12() {
        assert_eq!(
            (dev_isinf(f32::INFINITY), dev_isinf(-f32::INFINITY)),
            (1, -1)
        );
        assert_eq!(
            (dev_isnan(f32::NAN), dev_finite(1.0), dev_finite(f32::NAN)),
            (1, 1, 0)
        );
        assert_eq!(delta_angle(3.0, -3.0), 2.0f32 * std::f32::consts::PI - 6.0);
        let mut gl = g(UpstreamDefects::Fix);
        assert_eq!((nint(&mut gl, -0.5), nint(&mut gl, 0.5)), (-1, 1));
        assert_eq!((nint2(&mut gl, -0.5), nint2(&mut gl, 0.5)), (-1, 0));
        assert!(gl.notices.is_empty());
        assert_eq!(nint(&mut gl, 3e9), i32::MAX);
        assert_eq!(nint(&mut gl, f64::NAN), 0);
        assert!(gl.notices.contains(&FS_12));
        let mut gr = g(UpstreamDefects::Reproduce);
        assert_eq!(nint(&mut gr, 3e9), i32::MIN);
        assert_eq!(sum2stddev(6.0, 14.0, 3), 1.0);
        assert_eq!((compare_floats(f32::NAN, 1.0), compare_ints(2, 1)), (0, 1));
    }

    #[test]
    fn integer_lists_and_fs10() {
        let mut gl = g(UpstreamDefects::Fix);
        let mut l = vec![3, 1, 3, 2, 1, 3];
        assert_eq!(nunique_int_list(&mut gl, &mut l), 3);
        assert_eq!(l, [1, 1, 2, 3, 3, 3]);
        assert_eq!(unique_int_list(&mut gl, &mut l), [1, 2, 3]);
        assert_eq!(most_frequent_int_list(&mut gl, &mut [2, 1, 2, 1]), (1, 2));
        assert!(gl.notices.is_empty());
        assert_eq!(nunique_int_list(&mut gl, &mut []), 0);
        assert!(gl.notices.contains(&FS_10));
        let mut gr = g(UpstreamDefects::Reproduce);
        assert_eq!(nunique_int_list(&mut gr, &mut []), 1);
        assert_eq!(unique_int_list(&mut gr, &mut []), [0]);
    }

    #[test]
    fn order_statistics_and_fs09() {
        let mut gl = g(UpstreamDefects::Fix);
        let mut a = [5.0f32, 1.0, 4.0, 2.0, 3.0];
        assert_eq!(median(&mut gl, &mut a), 3.0);
        let mut a = [5.0f32, 1.0, 4.0, 2.0];
        assert_eq!(median(&mut gl, &mut a), 3.0);
        let mut a = [9.0f32, 7.0, 8.0, 1.0, 1.0, 0.5, 2.0, 2.0];
        assert_eq!(quick_select(&mut gl, &mut a, 1), 0.5);
        assert_eq!(kth_smallest(&mut [3.0, 1.0, 2.0], 3), 3.0);
        assert!(gl.notices.is_empty());
        assert!(median(&mut gl, &mut []).is_nan());
        assert!(gl.notices.contains(&FS_09));
        // NaNs stop the scans instead of running past the list.
        let mut a = [f32::NAN, 1.0, 3.0, f32::NAN, 2.0, 0.0];
        let _ = median(&mut gl, &mut a);
        // The even mean's NaN bits as the official build gives them (the open core probe's
        // `median`, observed 2026-11-14): of two NaNs the second selection's, quieted; the
        // default NaN (sign set) for inf + -inf.
        for (list, want) in [
            (&[0x7fc0_0001u32, 0x7fc0_0002][..], 0x7fc0_0002u32),
            (&[0x7f80_0001, 0x7fc0_0002], 0x7fc0_0002),
            (&[0x7fc0_0002, 0x7f80_0001], 0x7fc0_0001),
            (
                &[0xffc0_0005, 0x7fa0_0003, 0x3f80_0000, 0x4000_0000],
                0xffc0_0005,
            ),
            (&[0x3f80_0000, 0x4000_0000], 0x3fc0_0000),
            (&[0x7f80_0000, 0xff80_0000], 0xffc0_0000),
        ] {
            let mut a: Vec<f32> = list.iter().map(|&b| f32::from_bits(b)).collect();
            assert_eq!(median(&mut gl, &mut a).to_bits(), want, "{list:x?}");
        }
    }

    #[test]
    fn fast_atan2_table_and_fs06() {
        // The table's bytes: float(atan2(i, 100000)) for i = 0 .. 100000, recorded from glibc
        // 2.35's atan2 (CPython's math.atan2 on the main machine), the same in every entry as the
        // correctly rounded value.
        let bytes: Vec<u8> = atan_table().iter().flat_map(|v| v.to_le_bytes()).collect();
        use sha2::{Digest, Sha256};
        let hex: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            hex,
            "4bb06d9e2f3a1b01bf4256f7befbd5f9842036095048023174106d0ef7f8addf"
        );
        let env = Environment::from_pairs::<&str, &str>([]);
        let mut gl = g(UpstreamDefects::Fix);
        let mut out = Vec::new();
        let pi = std::f64::consts::PI as f32;
        assert_eq!(fast_approx_atan2f(&mut gl, &env, &mut out, 0.0, 0.0), 0.0);
        assert_eq!(fast_approx_atan2f(&mut gl, &env, &mut out, 0.0, -0.0), 0.0);
        let q = fast_approx_atan2f(&mut gl, &env, &mut out, 1.0, 1.0);
        assert_eq!(q, atan_table()[100_000]);
        assert_eq!(
            fast_approx_atan2f(&mut gl, &env, &mut out, -1.0, -1.0),
            (-f64::from(pi) + f64::from(q)) as f32
        );
        assert!(gl.notices.is_empty() && out.is_empty());
        // 1e35 * 100000 overflows a float: the scaled quotient (50000).
        assert_eq!(
            fast_approx_atan2f(&mut gl, &env, &mut out, 1e35, 2e35),
            atan_table()[50_000]
        );
        assert!(gl.notices.is_empty());
        assert!(fast_approx_atan2f(&mut gl, &env, &mut out, f32::NAN, 1.0).is_nan());
        assert!(gl.notices.contains(&FS_06));
        let env = Environment::from_pairs([("FREESURFER_fastApproxAtan2f_use_atanf2", "")]);
        let mut gl = g(UpstreamDefects::Fix);
        fast_approx_atan2f(&mut gl, &env, &mut out, 1.0, 2.0);
        fast_approx_atan2f(&mut gl, &env, &mut out, 1.0, 2.0);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!("{BUILD_ROOT}/utils/utils.cpp:1727 using slow atan2f\n")
        );
    }

    #[test]
    fn logic_problem_reports() {
        let mut gl = g(UpstreamDefects::Fix);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let calls: Vec<bool> = (0..130)
            .map(|_| {
                spend_time_checking_for_logic_problem(&mut gl, &mut out, &mut err, "f.c", 7)
                    .unwrap()
            })
            .collect();
        let at: Vec<usize> = (0..130).filter(|&i| calls[i]).map(|i| i + 1).collect();
        let expected: Vec<usize> = (1..=32).chain([64, 128]).collect();
        assert_eq!(at, expected);
        let env = Environment::from_pairs([("FREESURFER_reportLogicErrors", "1"), ("FIXIT", "")]);
        let r = cope_with_logic_problem2(
            &mut gl,
            &env,
            &mut out,
            &mut err,
            Some("FIXIT"),
            "m",
            "f.c",
            9,
            "fn",
        )
        .unwrap();
        assert_eq!(r, (LogicProblemResponse::Fix, true));
        cope_with_logic_problem2(
            &mut gl, &env, &mut out, &mut err, None, "m", "f.c", 10, "fn",
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(out.clone()).unwrap(),
            "f.c:9 fn - probable logic error: m, fixing it\n.  Set FIXIT to fix it\nf.c:10 fn - probable logic error: m, not fixing it\n"
        );
        let e = spend_time_checking_for_logic_problem(&mut gl, &mut out, &mut err, "f", 1_000_000);
        assert_eq!(e, Err(Fatal::Exit(1)));
    }

    #[test]
    fn temporary_files() {
        let base = tempfile::tempdir().unwrap();
        let b = base.path().to_string_lossy().into_owned();
        let env = Environment::from_pairs([("TMPDIR", b.as_str())]);
        let mut gl = g(UpstreamDefects::Fix);
        let mut err = Vec::new();
        assert_eq!(get_base_temp_dir(&mut gl, &env, None, &mut err).unwrap(), b);
        let f = make_temp_file(&mut gl, &env, None, &mut err, ".mgz").unwrap();
        assert!(
            f.starts_with(&format!("{b}/tmp.")) && f.ends_with(".mgz") && f.len() == b.len() + 15
        );
        assert!(Path::new(&f).is_file());
        let d = make_temp_dir(&mut gl, &env, None, &mut err).unwrap();
        assert!(Path::new(&d).is_dir());
        assert!(err.is_empty());
        // An unwritable variable is skipped; the last one's value (TEMPDIR) is what is left.
        let env = Environment::from_pairs([("FS_TMPDIR", "/nonexistent-dir-x")]);
        if directory_is_writable(None, "/tmp") {
            assert_eq!(
                get_base_temp_dir(&mut gl, &env, None, &mut err).unwrap(),
                "/tmp"
            );
        }
        let env =
            Environment::from_pairs([("TMPDIR", b.as_str()), ("TEMPDIR", "/nonexistent-dir-x")]);
        assert_eq!(get_base_temp_dir(&mut gl, &env, None, &mut err).unwrap(), b);
        let env = Environment::from_pairs([("TEMPDIR", "/nonexistent-dir-x")]);
        assert_eq!(
            get_base_temp_dir(&mut gl, &env, None, &mut err),
            Err(Fatal::Exit(1))
        );
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "error: Can not write to temporary directory /nonexistent-dir-x\n"
        );
        assert_eq!(scan_d(b"  12 kB"), Some(12));
        assert_eq!(scan_d(b"kB"), None);
    }
}
