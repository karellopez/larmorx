// SPDX-License-Identifier: Apache-2.0
//! The C library's broken-down times (spec §14): `gmtime`, `localtime` and `strftime("%c")` as
//! glibc 2.35 gives them in the C locale. Written from POSIX.1-2017 (the `TZ` variable, 8.3;
//! `localtime`, `strftime`), RFC 8536 (the zone files), the GNU C Library manual ("TZ
//! Variable") and black-box runs of the C library: the probe's `time` group and the cases of
//! `validation/investigations/freesurfer-cleanroom-core/tz_probe.sh`, which settled what the
//! documents leave open:
//!
//! - `TZ` unset is `:/etc/localtime`; empty (or a lone `:`) is UTC named `UTC`; one leading `:` is
//!   dropped; a zone file is an absolute path, or a name under `TZDIR` (unset or empty: the
//!   system's folder), with no second folder tried; a file that cannot be read as a zone file
//!   (missing, a folder, empty) leaves the string to the POSIX reading.
//! - The POSIX reading: a name of at least three letters (or `<...>` of at least three
//!   characters); an offset's hours are kept to 24, its minutes and seconds to 59; a name whose
//!   offset is missing gives that name at UTC; a string that has no valid first name gives UTC
//!   with an empty name. A daylight name without a rule (or with a lone `,`) takes the
//!   transitions of the zone file `posixrules` beside the others, its types renamed and moved to
//!   the string's offsets and its transition times moved as its indicators say, its own footer
//!   rule after its last transition; without that file the rule `M3.2.0,M11.1.0` (changes at
//!   02:00). A start rule without an end rule takes `M11.1.0`.
//! - A rule applies to the **UTC** year, and before 1970 the change times of 1970.
//! - Zone files: version 2 and later read their 64-bit data and footer; before the first
//!   transition the first standard-time type; after the last transition the footer rule; leap
//!   seconds apply to `gmtime` and `localtime` alike (the second of a leap second shows as 60).
//!
//! Malformed rules (an invalid month, week or day, a `/` without a time, characters after a
//! rule) make glibc keep a partly filled rule whose results this module does not follow: it reads
//! such a rule as the default one (the folder's README records the cases).

use crate::files::path_of;

/// A broken-down time (C's `struct tm`, with glibc's offset and zone name).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tm {
    /// Years since 1900.
    pub year: i32,
    /// Month, 0 to 11.
    pub mon: i32,
    /// Day of the month, 1 to 31.
    pub mday: i32,
    pub hour: i32,
    pub min: i32,
    /// Seconds, 0 to 60 (60 during a leap second).
    pub sec: i32,
    /// Day of the week, 0 (Sunday) to 6.
    pub wday: i32,
    /// Day of the year, 0 to 365.
    pub yday: i32,
    /// 1 in daylight time, else 0.
    pub isdst: i32,
    /// Seconds east of UTC.
    pub gmtoff: i64,
    /// The zone's abbreviation.
    pub zone: Vec<u8>,
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian; H. Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// The civil date (year, month 1 to 12, day) of a count of days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The UTC year of a time.
fn year_of(t: i64) -> i64 {
    civil_from_days(t.div_euclid(86400)).0
}

/// The broken-down form of `t + gmtoff` (None when the year does not fit C's `int`).
fn broken(t: i64, ty: &TType, leap_hit: bool) -> Option<Tm> {
    let local = t.checked_add(ty.off)?;
    let days = local.div_euclid(86400);
    let secs = local.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let year = i32::try_from(y - 1900).ok()?;
    let yday = days - days_from_civil(y, 1, 1);
    Some(Tm {
        year,
        mon: (m - 1) as i32,
        mday: d as i32,
        hour: (secs / 3600) as i32,
        min: (secs / 60 % 60) as i32,
        sec: (secs % 60) as i32 + i32::from(leap_hit),
        wday: (days + 4).rem_euclid(7) as i32,
        yday: yday as i32,
        isdst: i32::from(ty.isdst),
        gmtoff: ty.off,
        zone: ty.name.clone(),
    })
}

const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `strftime(..., "%c", tm)` in the C locale: `%a %b %e %H:%M:%S %Y`.
pub fn strftime_c(tm: &Tm) -> Vec<u8> {
    let day = DAYS.get(tm.wday as usize).copied().unwrap_or("?");
    let mon = MONTHS.get(tm.mon as usize).copied().unwrap_or("?");
    format!(
        "{day} {mon} {:>2} {:02}:{:02}:{:02} {}",
        tm.mday,
        tm.hour,
        tm.min,
        tm.sec,
        i64::from(tm.year) + 1900
    )
    .into_bytes()
}

/// A local time type: its offset east of UTC, daylight flag and abbreviation.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TType {
    off: i64,
    isdst: bool,
    name: Vec<u8>,
}

/// The day of a POSIX rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Day {
    /// `Jn`: 1 to 365, February 29th never counted.
    Julian(i64),
    /// `n`: 0 to 365, February 29th counted.
    Zero(i64),
    /// `Mm.w.d`: day d (0 Sunday) of week w (5 the last) of month m.
    Month(i64, i64, i64),
}

/// A POSIX change rule: its day and its local time of day (seconds, may be negative or beyond
/// a day).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rule {
    day: Day,
    time: i64,
}

impl Rule {
    const DEFAULT_START: Rule = Rule {
        day: Day::Month(3, 2, 0),
        time: 7200,
    };
    const DEFAULT_END: Rule = Rule {
        day: Day::Month(11, 1, 0),
        time: 7200,
    };

    /// The rule's local time in `year`, as seconds from 1970-01-01 00:00 of that local clock.
    fn local(&self, year: i64) -> i64 {
        let jan1 = days_from_civil(year, 1, 1);
        let day = match self.day {
            Day::Julian(n) => jan1 + n - 1 + i64::from(is_leap(year) && n >= 60),
            Day::Zero(n) => jan1 + n,
            Day::Month(m, w, d) => {
                let first = days_from_civil(year, m, 1);
                let wd1 = (first + 4).rem_euclid(7);
                let mut md = 1 + (d - wd1).rem_euclid(7) + 7 * (w - 1);
                while md > days_in_month(year, m) {
                    md -= 7;
                }
                first + md - 1
            }
        };
        day * 86400 + self.time
    }
}

/// A POSIX `TZ` string's zone: standard time, and daylight time with its two rules.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Posix {
    std: TType,
    dst: Option<(TType, Rule, Rule)>,
}

impl Posix {
    fn utc(name: &[u8]) -> Posix {
        Posix {
            std: TType {
                off: 0,
                isdst: false,
                name: name.to_vec(),
            },
            dst: None,
        }
    }

    /// The type in effect at `t`: the rules of `t`'s UTC year, or of 1970 before it.
    fn at(&self, t: i64) -> &TType {
        let Some((dst, start, end)) = &self.dst else {
            return &self.std;
        };
        let y = year_of(t).max(1970);
        let s = start.local(y) - self.std.off;
        let e = end.local(y) - dst.off;
        let in_dst = if s < e {
            s <= t && t < e
        } else {
            !(e <= t && t < s)
        };
        if in_dst { dst } else { &self.std }
    }
}

/// What the POSIX reading of a `TZ` string gave.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Reading {
    /// No valid first name: UTC with an empty name.
    Fail,
    /// A name without an offset: that name at UTC.
    NameOnly(Vec<u8>),
    /// Standard time (and daylight time with rules).
    Zone(Posix),
    /// A daylight name without rules: the standard and daylight types.
    NoRule(TType, TType),
}

fn parse_name(s: &[u8], i: usize) -> Option<(Vec<u8>, usize)> {
    if s.get(i) == Some(&b'<') {
        let mut j = i + 1;
        while j < s.len() && s[j] != b'>' {
            if !(s[j].is_ascii_alphanumeric() || s[j] == b'+' || s[j] == b'-') {
                return None;
            }
            j += 1;
        }
        if j >= s.len() || j - (i + 1) < 3 {
            return None;
        }
        return Some((s[i + 1..j].to_vec(), j + 1));
    }
    let mut j = i;
    while j < s.len() && s[j].is_ascii_alphabetic() {
        j += 1;
    }
    (j - i >= 3).then(|| (s[i..j].to_vec(), j))
}

fn digits(s: &[u8], mut i: usize) -> Option<(i64, usize)> {
    let start = i;
    let mut v: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add(i64::from(s[i] - b'0'));
        i += 1;
    }
    (i > start).then_some((v, i))
}

/// `[+-]hh[:mm[:ss]]` as signed seconds; the hours kept to `max_hours` when given, the minutes
/// and seconds to 59.
fn parse_hms(s: &[u8], mut i: usize, max_hours: Option<i64>) -> Option<(i64, usize)> {
    let mut sign = 1;
    match s.get(i) {
        Some(b'+') => i += 1,
        Some(b'-') => {
            sign = -1;
            i += 1;
        }
        _ => {}
    }
    let (mut h, j) = digits(s, i)?;
    if let Some(m) = max_hours {
        h = h.min(m);
    }
    i = j;
    let mut secs = h.saturating_mul(3600);
    for mult in [60, 1] {
        if s.get(i) != Some(&b':') {
            break;
        }
        match digits(s, i + 1) {
            Some((v, j)) => {
                secs += v.min(59) * mult;
                i = j;
            }
            None => break,
        }
    }
    Some((sign * secs, i))
}

fn parse_rule(s: &[u8], i: usize) -> Option<(Rule, usize)> {
    let (day, mut i) = match s.get(i)? {
        b'J' => {
            let (n, j) = digits(s, i + 1)?;
            if !(1..=365).contains(&n) {
                return None;
            }
            (Day::Julian(n), j)
        }
        b'M' => {
            let (m, j) = digits(s, i + 1)?;
            if s.get(j) != Some(&b'.') {
                return None;
            }
            let (w, j) = digits(s, j + 1)?;
            if s.get(j) != Some(&b'.') {
                return None;
            }
            let (d, j) = digits(s, j + 1)?;
            if !(1..=12).contains(&m) || !(1..=5).contains(&w) || !(0..=6).contains(&d) {
                return None;
            }
            (Day::Month(m, w, d), j)
        }
        c if c.is_ascii_digit() => {
            let (n, j) = digits(s, i)?;
            if n > 365 {
                return None;
            }
            (Day::Zero(n), j)
        }
        _ => return None,
    };
    let mut time = 7200;
    if s.get(i) == Some(&b'/') {
        let (t, j) = parse_hms(s, i + 1, None)?;
        time = t;
        i = j;
    }
    Some((Rule { day, time }, i))
}

/// The POSIX reading of a `TZ` string (after its leading `:`).
fn read_posix(s: &[u8]) -> Reading {
    let Some((std_name, i)) = parse_name(s, 0) else {
        return Reading::Fail;
    };
    let Some((west, i)) = parse_hms(s, i, Some(24)) else {
        return Reading::NameOnly(std_name);
    };
    let std = TType {
        off: -west,
        isdst: false,
        name: std_name,
    };
    if i >= s.len() {
        return Reading::Zone(Posix { std, dst: None });
    }
    let Some((dst_name, mut i)) = parse_name(s, i) else {
        // glibc keeps a partly filled daylight rule here; read as standard time alone
        return Reading::Zone(Posix { std, dst: None });
    };
    let mut dst_off = std.off + 3600;
    if let Some((w, j)) = parse_hms(s, i, Some(24)) {
        dst_off = -w;
        i = j;
    }
    let dst = TType {
        off: dst_off,
        isdst: true,
        name: dst_name,
    };
    let rest = &s[i..];
    if rest.is_empty() || rest == b"," || rest[0] != b',' {
        return Reading::NoRule(std, dst);
    }
    let (start, end) = match parse_rule(s, i + 1) {
        Some((start, j)) => match s.get(j) {
            Some(b',') => (
                start,
                parse_rule(s, j + 1).map_or(Rule::DEFAULT_END, |r| r.0),
            ),
            _ => (start, Rule::DEFAULT_END),
        },
        None => (Rule::DEFAULT_START, Rule::DEFAULT_END),
    };
    Reading::Zone(Posix {
        std,
        dst: Some((dst, start, end)),
    })
}

/// A time zone as the C library reads `TZ`: a zone file's transitions and footer rule, or a
/// POSIX rule alone, and the leap seconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zone {
    /// Transition times, ascending.
    trans: Vec<i64>,
    /// The type each transition goes into.
    idx: Vec<usize>,
    types: Vec<TType>,
    /// Per type: transition times given in standard time, in UT (RFC 8536's indicators).
    isstd: Vec<bool>,
    isut: Vec<bool>,
    /// The type before the first transition (the first standard-time type).
    first: usize,
    /// The rule after the last transition (or alone).
    footer: Option<Posix>,
    /// Leap seconds: (occurrence, correction from then on).
    leaps: Vec<(i64, i64)>,
}

/// The system's zone folder.
pub const ZONE_DIR: &[u8] = b"/usr/share/zoneinfo";

impl Zone {
    /// UTC with the given name.
    pub fn utc(name: &[u8]) -> Zone {
        Zone::rule(Posix::utc(name))
    }

    fn rule(p: Posix) -> Zone {
        Zone {
            trans: Vec::new(),
            idx: Vec::new(),
            types: vec![p.std.clone()],
            isstd: vec![false],
            isut: vec![false],
            first: 0,
            footer: Some(p),
            leaps: Vec::new(),
        }
    }

    /// The zone of a `TZ` value (`None`: unset) and `TZDIR` value, read as the C library reads
    /// them (the module's notes).
    pub fn from_tz(tz: Option<&[u8]>, tzdir: Option<&[u8]>) -> Zone {
        let dir = tzdir.filter(|d| !d.is_empty()).unwrap_or(ZONE_DIR);
        let v: &[u8] = match tz {
            None => b"/etc/localtime",
            Some(v) => {
                let v = v.strip_prefix(b":").unwrap_or(v);
                if v.is_empty() {
                    return Zone::utc(b"UTC");
                }
                v
            }
        };
        let file = if v.first() == Some(&b'/') {
            v.to_vec()
        } else {
            [dir, b"/", v].concat()
        };
        if let Some(z) = Zone::from_file(&file) {
            return z;
        }
        match read_posix(v) {
            Reading::Fail => Zone::utc(b""),
            Reading::NameOnly(n) => Zone::utc(&n),
            Reading::Zone(p) => Zone::rule(p),
            Reading::NoRule(std, dst) => match Zone::from_file(&[dir, b"/posixrules"].concat()) {
                Some(rules) => rules.renamed(&std, &dst),
                None => Zone::rule(Posix {
                    std,
                    dst: Some((dst, Rule::DEFAULT_START, Rule::DEFAULT_END)),
                }),
            },
        }
    }

    /// A zone file (RFC 8536), `None` when it cannot be read as one.
    pub fn from_file(path: &[u8]) -> Option<Zone> {
        let b = std::fs::read(path_of(path)).ok()?;
        Zone::from_tzif(&b)
    }

    /// A zone file's bytes.
    pub fn from_tzif(b: &[u8]) -> Option<Zone> {
        let (v1, ver) = header(b, 0)?;
        if ver >= b'2' {
            let size1 = v1.size(4);
            let (v2, _) = header(b, 44 + size1)?;
            let start = 44 + size1 + 44;
            let mut z = v2.parse(b, start, 8)?;
            let foot_at = start + v2.size(8);
            if b.get(foot_at) == Some(&b'\n') {
                let rest = &b[foot_at + 1..];
                if let Some(end) = rest.iter().position(|&c| c == b'\n') {
                    z.footer = footer_rule(&rest[..end]);
                }
            }
            Some(z)
        } else {
            v1.parse(b, 44, 4)
        }
    }

    /// The `posixrules` transitions under a string's standard and daylight types: every type
    /// becomes one of the two by its daylight flag, and each transition moves by the change of
    /// the offset its indicators name (none in UT; the daylight one after daylight time given
    /// in wall-clock time; else the standard one), the file's own footer kept.
    fn renamed(&self, std: &TType, dst: &TType) -> Zone {
        let theirs = |want_dst: bool| {
            self.idx
                .iter()
                .rev()
                .map(|&k| &self.types[k])
                .find(|t| t.isdst == want_dst)
                .map(|t| t.off)
        };
        let their_std = theirs(false).unwrap_or(self.types[self.first].off);
        let their_dst = theirs(true).unwrap_or(their_std + 3600);
        let types: Vec<TType> = self
            .types
            .iter()
            .map(|t| if t.isdst { dst.clone() } else { std.clone() })
            .collect();
        let mut trans = self.trans.clone();
        for (i, t) in trans.iter_mut().enumerate() {
            let next = self.idx[i];
            let prev = if i == 0 { self.first } else { self.idx[i - 1] };
            if self.isut[next] {
                continue;
            }
            *t += if self.types[prev].isdst && !self.isstd[next] {
                dst.off - their_dst
            } else {
                std.off - their_std
            };
        }
        Zone {
            trans,
            idx: self.idx.clone(),
            types,
            isstd: self.isstd.clone(),
            isut: self.isut.clone(),
            first: self.first,
            footer: self.footer.clone(),
            leaps: self.leaps.clone(),
        }
    }

    /// The leap-second correction at `t` and whether `t` is a leap second itself.
    fn leap(&self, t: i64) -> (i64, bool) {
        let k = self.leaps.partition_point(|&(o, _)| o <= t);
        if k == 0 {
            return (0, false);
        }
        let (o, c) = self.leaps[k - 1];
        let before = if k >= 2 { self.leaps[k - 2].1 } else { 0 };
        (c, o == t && c > before)
    }

    fn type_at(&self, t: i64) -> &TType {
        match (self.trans.first(), self.trans.last(), &self.footer) {
            (None, _, Some(f)) => f.at(t),
            (None, _, None) => &self.types[self.first],
            (Some(&t0), _, _) if t < t0 => &self.types[self.first],
            (_, Some(&tl), Some(f)) if t >= tl => f.at(t),
            _ => {
                let k = self.trans.partition_point(|&x| x <= t);
                &self.types[self.idx[k - 1]]
            }
        }
    }

    /// `gmtime_r(t)` (the zone's leap seconds applied, as glibc does).
    pub fn gmtime(&self, t: i64) -> Option<Tm> {
        let (corr, hit) = self.leap(t);
        let gmt = TType {
            off: 0,
            isdst: false,
            name: b"GMT".to_vec(),
        };
        broken(t.checked_sub(corr)?, &gmt, hit)
    }

    /// `localtime_r(t)`.
    pub fn localtime(&self, t: i64) -> Option<Tm> {
        let (corr, hit) = self.leap(t);
        broken(t.checked_sub(corr)?, self.type_at(t), hit)
    }
}

/// A zone file's footer as a rule (`None` when empty or unreadable).
fn footer_rule(s: &[u8]) -> Option<Posix> {
    match read_posix(s) {
        Reading::Fail => None,
        Reading::NameOnly(n) => Some(Posix::utc(&n)),
        Reading::Zone(p) => Some(p),
        Reading::NoRule(std, dst) => Some(Posix {
            std,
            dst: Some((dst, Rule::DEFAULT_START, Rule::DEFAULT_END)),
        }),
    }
}

/// A TZif header's counts.
struct Counts {
    isut: usize,
    isstd: usize,
    leap: usize,
    time: usize,
    types: usize,
    chars: usize,
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn header(b: &[u8], at: usize) -> Option<(Counts, u8)> {
    if b.get(at..at + 4)? != b"TZif" {
        return None;
    }
    let ver = *b.get(at + 4)?;
    let c = |k: usize| be32(b, at + 20 + 4 * k).map(|v| v as usize);
    Some((
        Counts {
            isut: c(0)?,
            isstd: c(1)?,
            leap: c(2)?,
            time: c(3)?,
            types: c(4)?,
            chars: c(5)?,
        },
        ver,
    ))
}

impl Counts {
    /// The data block's size with `w`-byte times.
    fn size(&self, w: usize) -> usize {
        self.time * (w + 1)
            + self.types * 6
            + self.chars
            + self.leap * (w + 4)
            + self.isstd
            + self.isut
    }

    fn parse(&self, b: &[u8], start: usize, w: usize) -> Option<Zone> {
        if self.types == 0
            || (self.isstd != 0 && self.isstd != self.types)
            || (self.isut != 0 && self.isut != self.types)
            || b.len() < start + self.size(w)
        {
            return None;
        }
        let time = |at: usize| -> i64 {
            if w == 8 {
                i64::from_be_bytes(b[at..at + 8].try_into().unwrap_or([0; 8]))
            } else {
                i64::from(i32::from_be_bytes(
                    b[at..at + 4].try_into().unwrap_or([0; 4]),
                ))
            }
        };
        let mut p = start;
        let trans: Vec<i64> = (0..self.time).map(|k| time(p + k * w)).collect();
        p += self.time * w;
        let idx: Vec<usize> = b[p..p + self.time].iter().map(|&k| k as usize).collect();
        p += self.time;
        if idx.iter().any(|&k| k >= self.types) {
            return None;
        }
        let ttinfo: Vec<(i64, bool, usize)> = (0..self.types)
            .map(|k| {
                let q = p + 6 * k;
                let off = i32::from_be_bytes(b[q..q + 4].try_into().unwrap_or([0; 4]));
                (i64::from(off), b[q + 4] != 0, b[q + 5] as usize)
            })
            .collect();
        p += 6 * self.types;
        let chars = &b[p..p + self.chars];
        p += self.chars;
        let mut types = Vec::with_capacity(self.types);
        for (off, isdst, ab) in ttinfo {
            if ab >= chars.len().max(1) && !(ab == 0 && chars.is_empty()) {
                return None;
            }
            let tail = chars.get(ab..).unwrap_or(&[]);
            let end = tail.iter().position(|&c| c == 0).unwrap_or(tail.len());
            types.push(TType {
                off,
                isdst,
                name: tail[..end].to_vec(),
            });
        }
        let leaps: Vec<(i64, i64)> = (0..self.leap)
            .map(|k| {
                let q = p + k * (w + 4);
                let c = i32::from_be_bytes(b[q + w..q + w + 4].try_into().unwrap_or([0; 4]));
                (time(q), i64::from(c))
            })
            .collect();
        p += self.leap * (w + 4);
        let flags = |n: usize, at: usize| -> Vec<bool> {
            if n == 0 {
                vec![false; self.types]
            } else {
                b[at..at + n].iter().map(|&c| c != 0).collect()
            }
        };
        let isstd = flags(self.isstd, p);
        let isut = flags(self.isut, p + self.isstd);
        let first = types.iter().position(|t| !t.isdst).unwrap_or(0);
        Some(Zone {
            trans,
            idx,
            types,
            isstd,
            isut,
            first,
            footer: None,
            leaps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip() {
        for z in [-800_000i64, -25567, -1, 0, 1, 10957, 47482, 2_932_896] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-25567), (1900, 1, 1));
    }

    #[test]
    fn posix_rules() {
        let z = Zone::from_tz(Some(b"EST5EDT,M3.2.0,M11.1.0"), Some(b"/nonexistent"));
        // 2023-03-12 07:00 UTC: daylight time starts
        assert_eq!(z.localtime(1678604399).unwrap().isdst, 0);
        assert_eq!(z.localtime(1678604400).unwrap().isdst, 1);
        // 2023-11-05 06:00 UTC: it ends
        assert_eq!(z.localtime(1699163999).unwrap().isdst, 1);
        assert_eq!(z.localtime(1699164000).unwrap().isdst, 0);
        // never before 1970 (the change times of 1970)
        assert_eq!(z.localtime(-1_000_000_000).unwrap().isdst, 0);
        let s = Zone::from_tz(Some(b"AEST-10AEDT,M10.1.0,M4.1.0/3"), Some(b"/nonexistent"));
        assert_eq!(s.localtime(-1_000_000_000).unwrap().isdst, 1);
        let q = Zone::from_tz(Some(b"<+0530>-5:30"), Some(b"/nonexistent"));
        let tm = q.localtime(0).unwrap();
        assert_eq!((tm.hour, tm.min, tm.gmtoff), (5, 30, 19800));
        assert_eq!(tm.zone, b"+0530");
    }

    #[test]
    fn readings() {
        let none = Some(&b"/nonexistent"[..]);
        assert_eq!(
            Zone::from_tz(Some(b"No/Such_Zone"), none)
                .localtime(0)
                .unwrap()
                .zone,
            b""
        );
        assert_eq!(
            Zone::from_tz(Some(b"Foo"), none).localtime(0).unwrap().zone,
            b"Foo"
        );
        assert_eq!(
            Zone::from_tz(Some(b""), none).localtime(0).unwrap().zone,
            b"UTC"
        );
        assert_eq!(
            Zone::from_tz(Some(b":"), none).localtime(0).unwrap().zone,
            b"UTC"
        );
        assert_eq!(
            Zone::from_tz(Some(b"FOO+25"), none)
                .localtime(0)
                .unwrap()
                .gmtoff,
            -86400
        );
        assert_eq!(
            Zone::from_tz(Some(b"EST5:75"), none)
                .localtime(0)
                .unwrap()
                .gmtoff,
            -21540
        );
    }

    #[test]
    fn percent_c() {
        let tm = Zone::utc(b"UTC").localtime(0).unwrap();
        assert_eq!(strftime_c(&tm), b"Thu Jan  1 00:00:00 1970");
    }
}
