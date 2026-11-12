// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The C library's broken-down times as FreeSurfer prints them (`gmtime_r`, `localtime`,
//! `strftime("%c")` in the C locale): larmorx's own code, written from POSIX (the `TZ`
//! variable's format, XBD 8.3), RFC 8536 (the TZif files of the time-zone database) and
//! glibc's documented choice of the zone (no C library source).
//!
//! The zone of `localtime` comes from the program's `TZ` (its [`Environment`]): unset, the file
//! `/etc/localtime`; empty, UTC; otherwise (after one leading `:`) a TZif file (an absolute
//! path, or a name under `TZDIR`, else `/usr/share/zoneinfo`), and when no such file can be read,
//! a POSIX `TZ` string; anything unreadable, UTC. Beyond a TZif file's last transition its
//! footer's `TZ` string applies (RFC 8536 §3.3).

use std::path::PathBuf;

use crate::run::Environment;

/// `struct tm`'s fields that FreeSurfer prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tm {
    /// Years since 1900.
    pub year: i64,
    /// 0 to 11.
    pub mon: u32,
    /// 1 to 31.
    pub mday: u32,
    pub hour: u32,
    pub min: u32,
    pub sec: u32,
    /// 0 (Sunday) to 6.
    pub wday: u32,
}

/// Days since 1970-01-01 to (year, month 1-12, day): Howard Hinnant's civil-from-days.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month 1-12, day) to days since 1970-01-01.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `gmtime_r(&t)`.
pub fn gmtime(t: i64) -> Tm {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400) as u32;
    let (y, m, d) = civil_from_days(days);
    Tm {
        year: y - 1900,
        mon: m - 1,
        mday: d,
        hour: secs / 3600,
        min: secs / 60 % 60,
        sec: secs % 60,
        wday: (days + 4).rem_euclid(7) as u32,
    }
}

/// `localtime(&t)` in the program's zone.
pub fn localtime(env: &Environment, t: i64) -> Tm {
    gmtime(t + Zone::of(env).offset_at(t))
}

/// `strftime(buf, n, "%c", tm)` in the C locale: `%a %b %e %H:%M:%S %Y`.
pub fn strftime_c(tm: &Tm) -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{} {} {:2} {:02}:{:02}:{:02} {}",
        DAYS[tm.wday as usize],
        MONTHS[tm.mon as usize],
        tm.mday,
        tm.hour,
        tm.min,
        tm.sec,
        tm.year + 1900
    )
}

/// The seconds since the epoch now (`time(NULL)`).
pub fn now() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    }
}

/// A day rule of a POSIX `TZ` string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Day {
    /// `Jn`: 1 to 365, February 29 never counted.
    Julian1(u32),
    /// `n`: 0 to 365, February 29 counted.
    Julian0(u32),
    /// `Mm.w.d`.
    Month(u32, u32, u32),
}

/// A parsed POSIX `TZ` string: standard offset (seconds east of UTC) and, with daylight
/// saving, its offset and the two rules with their local times.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PosixTz {
    std_off: i64,
    dst: Option<(i64, Day, i64, Day, i64)>,
}

struct Cursor<'a> {
    s: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn name(&mut self) -> Option<()> {
        if self.peek() == Some(b'<') {
            let end = self.s[self.i..].iter().position(|&b| b == b'>')?;
            self.i += end + 1;
            return Some(());
        }
        let start = self.i;
        while self.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
            self.i += 1;
        }
        (self.i - start >= 3).then_some(())
    }
    fn num(&mut self) -> Option<i64> {
        let start = self.i;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()?
            .parse()
            .ok()
    }
    /// `[+-]hh[:mm[:ss]]` in seconds.
    fn hms(&mut self) -> Option<i64> {
        let sign = match self.peek() {
            Some(b'-') => {
                self.i += 1;
                -1
            }
            Some(b'+') => {
                self.i += 1;
                1
            }
            _ => 1,
        };
        let mut v = self.num()? * 3600;
        for scale in [60, 1] {
            if self.peek() != Some(b':') {
                break;
            }
            self.i += 1;
            v += self.num()? * scale;
        }
        Some(sign * v)
    }
    fn day(&mut self) -> Option<Day> {
        match self.peek()? {
            b'J' => {
                self.i += 1;
                Some(Day::Julian1(self.num()? as u32))
            }
            b'M' => {
                self.i += 1;
                let m = self.num()? as u32;
                (self.peek()? == b'.').then_some(())?;
                self.i += 1;
                let w = self.num()? as u32;
                (self.peek()? == b'.').then_some(())?;
                self.i += 1;
                let d = self.num()? as u32;
                (1..=12).contains(&m).then_some(Day::Month(m, w, d))
            }
            _ => Some(Day::Julian0(self.num()? as u32)),
        }
    }
    fn rule(&mut self) -> Option<(Day, i64)> {
        let d = self.day()?;
        let t = if self.peek() == Some(b'/') {
            self.i += 1;
            self.hms()?
        } else {
            7200
        };
        Some((d, t))
    }
}

impl PosixTz {
    fn parse(s: &str) -> Option<PosixTz> {
        let mut c = Cursor {
            s: s.as_bytes(),
            i: 0,
        };
        c.name()?;
        let std_off = -c.hms()?;
        if c.peek().is_none() {
            return Some(PosixTz { std_off, dst: None });
        }
        c.name()?;
        let dst_off = match c.peek() {
            Some(b',') | None => std_off + 3600,
            _ => -c.hms()?,
        };
        let (start, end) = if c.peek() == Some(b',') {
            c.i += 1;
            let start = c.rule()?;
            (c.peek()? == b',').then_some(())?;
            c.i += 1;
            (start, c.rule()?)
        } else {
            // No rule: the United States' rule.
            ((Day::Month(3, 2, 0), 7200), (Day::Month(11, 1, 0), 7200))
        };
        if c.peek().is_some() {
            return None;
        }
        Some(PosixTz {
            std_off,
            dst: Some((dst_off, start.0, start.1, end.0, end.1)),
        })
    }

    /// The day of the year (0-based) of a rule in `year`.
    fn yday(year: i64, day: Day) -> i64 {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        match day {
            Day::Julian1(n) => {
                let n = i64::from(n) - 1;
                if leap && n >= 59 { n + 1 } else { n }
            }
            Day::Julian0(n) => i64::from(n),
            Day::Month(m, w, d) => {
                let first = days_from_civil(year, m, 1);
                let jan1 = days_from_civil(year, 1, 1);
                let wday_first = (first + 4).rem_euclid(7);
                let mut mday =
                    1 + (i64::from(d) - wday_first).rem_euclid(7) + 7 * (i64::from(w) - 1);
                let len =
                    days_from_civil(if m == 12 { year + 1 } else { year }, m % 12 + 1, 1) - first;
                while mday > len {
                    mday -= 7;
                }
                first - jan1 + mday - 1
            }
        }
    }

    fn offset_at(&self, t: i64) -> i64 {
        let Some((dst_off, sd, st, ed, et)) = self.dst else {
            return self.std_off;
        };
        // The rules apply to the UTC year; before 1970 the C library (glibc 2.35, measured:
        // validation/investigations/freesurfer-core/ctime_probe.py) uses 1970's change times.
        let year = (gmtime(t).year + 1900).max(1970);
        let jan1 = days_from_civil(year, 1, 1) * 86_400;
        let start = jan1 + Self::yday(year, sd) * 86_400 + st - self.std_off;
        let end = jan1 + Self::yday(year, ed) * 86_400 + et - dst_off;
        let in_dst = if start < end {
            t >= start && t < end
        } else {
            !(t >= end && t < start)
        };
        if in_dst { dst_off } else { self.std_off }
    }
}

/// A TZif file's transitions, types and footer (RFC 8536).
#[derive(Clone, Debug, Default, PartialEq)]
struct TzFile {
    times: Vec<i64>,
    idx: Vec<u8>,
    types: Vec<(i64, bool)>,
    footer: Option<PosixTz>,
}

fn be(b: &[u8], n: usize) -> Option<u64> {
    let s = b.get(..n)?;
    Some(s.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
}

impl TzFile {
    fn parse(b: &[u8]) -> Option<TzFile> {
        let counts = |h: &[u8]| -> Option<[usize; 6]> {
            (h.get(..4)? == b"TZif").then_some(())?;
            let mut c = [0usize; 6];
            for (k, v) in c.iter_mut().enumerate() {
                *v = be(h.get(20 + 4 * k..)?, 4)? as usize;
            }
            Some(c)
        };
        // isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt.
        let c1 = counts(b)?;
        let v1_len = c1[3] * 5 + c1[4] * 6 + c1[5] + c1[2] * 8 + c1[1] + c1[0];
        let version2 = *b.get(4)? >= b'2';
        let (h, width) = if version2 {
            (b.get(44 + v1_len..)?, 8)
        } else {
            (b, 4)
        };
        let [isut, isstd, leap, timecnt, typecnt, charcnt] = counts(h)?;
        let body = h.get(44..)?;
        let mut f = TzFile::default();
        for k in 0..timecnt {
            let raw = be(body.get(k * width..)?, width)?;
            f.times.push(if width == 8 {
                raw as i64
            } else {
                i64::from(raw as u32 as i32)
            });
        }
        f.idx = body.get(timecnt * width..timecnt * (width + 1))?.to_vec();
        let tt = timecnt * (width + 1);
        for k in 0..typecnt {
            let off = be(body.get(tt + 6 * k..)?, 4)? as u32 as i32;
            f.types
                .push((i64::from(off), *body.get(tt + 6 * k + 4)? != 0));
        }
        let len = tt + typecnt * 6 + charcnt + leap * (width + 4) + isstd + isut;
        if version2 {
            let footer = body.get(len..)?;
            if footer.first() == Some(&b'\n') {
                let end = footer[1..].iter().position(|&x| x == b'\n')?;
                let s = std::str::from_utf8(&footer[1..1 + end]).ok()?;
                f.footer = PosixTz::parse(s);
            }
        }
        (!f.types.is_empty() && f.idx.iter().all(|&i| (i as usize) < typecnt)).then_some(f)
    }

    fn offset_at(&self, t: i64) -> i64 {
        if self.times.is_empty() || t < self.times[0] {
            let k = self.types.iter().position(|ty| !ty.1).unwrap_or(0);
            return self.types[k].0;
        }
        if t >= *self.times.last().expect("not empty")
            && let Some(f) = &self.footer
        {
            return f.offset_at(t);
        }
        let k = self.times.partition_point(|&x| x <= t) - 1;
        self.types[self.idx[k] as usize].0
    }
}

/// The program's time zone.
#[derive(Clone, Debug, PartialEq)]
enum Zone {
    Utc,
    File(TzFile),
    Posix(PosixTz),
}

impl Zone {
    fn of(env: &Environment) -> Zone {
        let tz = env.get("TZ").map(|v| v.to_string_lossy().into_owned());
        let spec = match tz.as_deref() {
            None => None,
            Some("") => return Zone::Utc,
            Some(s) => Some(s.strip_prefix(':').unwrap_or(s).to_owned()),
        };
        let path = match spec.as_deref() {
            None => PathBuf::from("/etc/localtime"),
            Some("") => return Zone::Utc,
            Some(s) if s.starts_with('/') => PathBuf::from(s),
            Some(s) => {
                let dir = env
                    .get("TZDIR")
                    .filter(|d| !d.is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/usr/share/zoneinfo"));
                dir.join(s)
            }
        };
        if let Some(f) = std::fs::read(&path).ok().and_then(|b| TzFile::parse(&b)) {
            return Zone::File(f);
        }
        match spec.as_deref().and_then(PosixTz::parse) {
            Some(p) => Zone::Posix(p),
            None => Zone::Utc,
        }
    }

    fn offset_at(&self, t: i64) -> i64 {
        match self {
            Zone::Utc => 0,
            Zone::File(f) => f.offset_at(t),
            Zone::Posix(p) => p.offset_at(t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmtime_and_c_format() {
        let tm = gmtime(1_793_000_000);
        assert_eq!(strftime_c(&tm), "Mon Oct 26 07:33:20 2026");
        assert_eq!(strftime_c(&gmtime(0)), "Thu Jan  1 00:00:00 1970");
        assert_eq!(strftime_c(&gmtime(-1)), "Wed Dec 31 23:59:59 1969");
        assert_eq!(gmtime(951_782_400).mday, 29); // 2000-02-29
    }

    #[test]
    fn posix_tz_strings() {
        let env = |tz: &str| Environment::from_pairs([("TZ", tz), ("TZDIR", "/nonexistent")]);
        // 2026-07-01 12:00 UTC and 2026-01-15 12:00 UTC.
        let (summer, winter) = (1_782_907_200, 1_768_478_400);
        let ny = env("EST5EDT,M3.2.0,M11.1.0");
        assert_eq!(localtime(&ny, summer).hour, 8);
        assert_eq!(localtime(&ny, winter).hour, 7);
        // DST starts 2026-03-08 at 02:00 EST = 07:00 UTC.
        let start = days_from_civil(2026, 3, 8) * 86_400 + 7 * 3600;
        assert_eq!(localtime(&ny, start - 1).hour, 1);
        assert_eq!(localtime(&ny, start).hour, 3);
        let syd = env("AEST-10AEDT,M10.1.0,M4.1.0/3");
        assert_eq!(localtime(&syd, winter).hour, 23);
        assert_eq!(localtime(&syd, summer).hour, 22);
        assert_eq!(localtime(&env("<+0530>-5:30"), 0).min, 30);
        assert_eq!(localtime(&env(""), summer).hour, 12);
        assert_eq!(localtime(&env("garbage1x"), summer).hour, 12);
    }

    #[test]
    fn tzif_file_with_footer() {
        // A version-2 file: one transition at 0 to type 1 (UTC+1), footer "CET-1".
        let mut b = Vec::new();
        let header = |b: &mut Vec<u8>, timecnt: u32, typecnt: u32, charcnt: u32| {
            b.extend_from_slice(b"TZif2");
            b.extend_from_slice(&[0; 15]);
            for c in [0u32, 0, 0, timecnt, typecnt, charcnt] {
                b.extend_from_slice(&c.to_be_bytes());
            }
        };
        header(&mut b, 1, 2, 4);
        b.extend_from_slice(&0i32.to_be_bytes());
        b.push(1);
        b.extend_from_slice(&0i32.to_be_bytes());
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&3600i32.to_be_bytes());
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(b"CET\0");
        header(&mut b, 1, 2, 4);
        b.extend_from_slice(&0i64.to_be_bytes());
        b.push(1);
        b.extend_from_slice(&0i32.to_be_bytes());
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&3600i32.to_be_bytes());
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(b"CET\0");
        b.extend_from_slice(b"\nCET-1CEST,M3.5.0,M10.5.0/3\n");
        let f = TzFile::parse(&b).unwrap();
        assert_eq!(f.offset_at(-10), 0);
        assert_eq!(f.offset_at(0), 3600);
        assert_eq!(f.offset_at(1_782_907_200), 7200);
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("Test"), &b).unwrap();
        let env = Environment::from_pairs([
            ("TZ", ":Test".to_owned()),
            ("TZDIR", d.path().to_string_lossy().into_owned()),
        ]);
        assert_eq!(localtime(&env, 1_768_478_400).hour, 13);
    }
}
