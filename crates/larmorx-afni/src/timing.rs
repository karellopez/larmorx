//! Slice acquisition times: AFNI's named patterns, the NIfTI header's slice-timing fields,
//! and the default time origin (`specs/3dTshift.md` §3).
//!
//! All times are float32, built by adding the slice step repeatedly, as AFNI does (observed:
//! the results match AFNI bit for bit).

use larmorx_io::nifti::NiftiHeader;

/// A named slice-timing pattern (`-tpattern`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// `alt+z`, `altplus`: 0, 2, 4, …, then 1, 3, 5, …
    AltPlus,
    /// `alt+z2`: 1, 3, 5, …, then 0, 2, 4, …
    AltPlus2,
    /// `alt-z`, `altminus`: nz−1, nz−3, …, then nz−2, nz−4, …
    AltMinus,
    /// `alt-z2`: nz−2, nz−4, …, then nz−1, nz−3, …
    AltMinus2,
    /// `seq+z`, `seqplus`: 0, 1, 2, …
    SeqPlus,
    /// `seq-z`, `seqminus`: nz−1, nz−2, …
    SeqMinus,
    /// `zero`, `simult`: every slice at 0.
    Zero,
}

impl Pattern {
    /// The pattern AFNI names `name` (case-sensitive, except `zero` and `simult`).
    pub fn from_name(name: &str) -> Option<Pattern> {
        Some(match name {
            "alt+z" | "altplus" => Pattern::AltPlus,
            "alt+z2" => Pattern::AltPlus2,
            "alt-z" | "altminus" => Pattern::AltMinus,
            "alt-z2" => Pattern::AltMinus2,
            "seq+z" | "seqplus" => Pattern::SeqPlus,
            "seq-z" | "seqminus" => Pattern::SeqMinus,
            _ if name.eq_ignore_ascii_case("zero") || name.eq_ignore_ascii_case("simult") => {
                Pattern::Zero
            }
            _ => return None,
        })
    }

    /// The slices in acquisition order.
    fn order(self, nz: usize) -> Vec<usize> {
        // Every other slice upwards from `first`, or downwards from the top of `0..below`.
        let up = |first: usize| (first..nz).step_by(2);
        let down = |below: usize| (0..below).rev().step_by(2);
        match self {
            Pattern::AltPlus => up(0).chain(up(1)).collect(),
            Pattern::AltPlus2 => up(1).chain(up(0)).collect(),
            Pattern::AltMinus => down(nz).chain(down(nz.saturating_sub(1))).collect(),
            Pattern::AltMinus2 => down(nz.saturating_sub(1)).chain(down(nz)).collect(),
            Pattern::SeqPlus => (0..nz).collect(),
            Pattern::SeqMinus => (0..nz).rev().collect(),
            Pattern::Zero => Vec::new(),
        }
    }

    /// Slice times for `nz` slices over `tr`: the step `tr/nz` (float32) added repeatedly in
    /// acquisition order.
    pub fn times(self, nz: usize, tr: f32) -> Vec<f32> {
        let mut times = vec![0f32; nz];
        if nz > 0 {
            assign(&mut times, &self.order(nz), tr / nz as f32);
        }
        times
    }
}

/// Gives the slices in `order` the times 0, step, step+step, … (float32 additions).
fn assign(times: &mut [f32], order: &[usize], step: f32) {
    let mut t = 0f32;
    for &k in order {
        times[k] = t;
        t += step;
    }
}

/// `tzero` by default: the mean slice time, summed and divided in float32.
pub fn mean_time(times: &[f32]) -> f32 {
    let mut sum = 0f32;
    for &t in times {
        sum += t;
    }
    sum / times.len() as f32
}

/// Slice timing read from a NIfTI header (observed rules, `specs/3dTshift.md` §3.2).
#[derive(Clone, Debug, PartialEq)]
pub struct HeaderTiming {
    /// `slice_code` (1 to 127).
    pub code: i32,
    pub start: usize,
    pub end: usize,
    /// `slice_duration` in the header's time unit converted to seconds.
    pub duration: f32,
    /// One time per slice (seconds).
    pub times: Vec<f32>,
}

/// How many of the header's time units (`xyzt_units`) make a second: 1000 for milliseconds,
/// 10⁶ for microseconds, and 1 for every other unit, which AFNI takes as seconds (observed).
pub fn time_units_per_second(hdr: &NiftiHeader) -> f64 {
    match hdr.xyzt_units & 0x38 {
        16 => 1e3,
        24 => 1e6,
        _ => 1.0,
    }
}

/// A header time value in seconds (divided in double, then rounded to float32).
pub fn to_seconds(value: f64, units_per_second: f64) -> f32 {
    (value / units_per_second) as f32
}

/// The slice times AFNI reads from the header's `slice_code`, `slice_start`, `slice_end`,
/// `slice_duration` and `dim_info` for `nz` slices, or `None` if the header gives none.
///
/// Observed with AFNI 25.2.09: the slice axis (`dim_info` bits 4–5) must be the third; the
/// code, read as a signed byte, must be positive; the duration must be positive; and
/// `0 ≤ slice_start < slice_end < nz`. Codes 1, 3, 4, 5 and 6 order the slices as NIfTI
/// defines (sequential and alternating, increasing and decreasing); code 2 (`SEQ_DEC`) and
/// codes 7 to 127 give every slice time 0. Slices outside `[slice_start, slice_end]` get 0.
pub fn header_timing(hdr: &NiftiHeader, nz: usize) -> Option<HeaderTiming> {
    let slice_dim = (hdr.dim_info >> 4) & 3;
    let code = i32::from(hdr.slice_code as u8 as i8);
    let duration = to_seconds(hdr.slice_duration, time_units_per_second(hdr));
    let (start, end) = (hdr.slice_start, hdr.slice_end);
    if slice_dim != 3 || code <= 0 || duration.is_nan() || duration <= 0.0 {
        return None;
    }
    if start < 0 || start >= end || end >= nz as i64 {
        return None;
    }
    let (start, end) = (start as usize, end as usize);
    // Every other slice upwards from `first`, or downwards from `top`, within the range.
    let up = |first: usize| (first..=end).step_by(2);
    let down = |top: usize| (start..=top).rev().step_by(2);
    let order: Vec<usize> = match code {
        1 => (start..=end).collect(),
        3 => up(start).chain(up(start + 1)).collect(),
        4 => down(end).chain(down(end - 1)).collect(),
        5 => up(start + 1).chain(up(start)).collect(),
        6 => down(end - 1).chain(down(end)).collect(),
        _ => Vec::new(),
    };
    let mut times = vec![0f32; nz];
    assign(&mut times, &order, duration);
    Some(HeaderTiming {
        code,
        start,
        end,
        duration,
        times,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::element::DataType;
    use larmorx_io::nifti::NiftiVersion;

    #[test]
    fn patterns_match_afni_help_table() {
        // AFNI's -help: nz = 5, TR = 1 (dt = 0.2).
        let cases = [
            (Pattern::AltPlus, [0.0, 0.6, 0.2, 0.8, 0.4]),
            (Pattern::AltPlus2, [0.4, 0.0, 0.6, 0.2, 0.8]),
            (Pattern::AltMinus, [0.4, 0.8, 0.2, 0.6, 0.0]),
            (Pattern::AltMinus2, [0.8, 0.2, 0.6, 0.0, 0.4]),
            (Pattern::SeqPlus, [0.0, 0.2, 0.4, 0.6, 0.8]),
            (Pattern::SeqMinus, [0.8, 0.6, 0.4, 0.2, 0.0]),
        ];
        for (pattern, expected) in cases {
            let times = pattern.times(5, 1.0);
            for (t, e) in times.iter().zip(expected) {
                assert!((t - e).abs() < 1e-6, "{pattern:?}: {times:?}");
            }
        }
        assert_eq!(Pattern::Zero.times(3, 2.0), [0.0; 3]);
    }

    #[test]
    fn times_are_repeated_float32_sums() {
        let times = Pattern::SeqPlus.times(33, 2.0);
        let step = 2.0f32 / 33.0;
        let mut t = 0f32;
        for &v in &times {
            assert_eq!(v.to_bits(), t.to_bits());
            t += step;
        }
    }

    #[test]
    fn names_are_case_sensitive_except_zero() {
        assert_eq!(Pattern::from_name("alt+z"), Some(Pattern::AltPlus));
        assert_eq!(Pattern::from_name("ALT+Z"), None);
        assert_eq!(Pattern::from_name("SIMULT"), Some(Pattern::Zero));
        assert_eq!(Pattern::from_name("Zero"), Some(Pattern::Zero));
    }

    fn header(code: u8, start: i64, end: i64, duration: f64, dim_info: u8) -> NiftiHeader {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[2, 2, 6, 10], DataType::F32).unwrap();
        h.slice_code = i32::from(code);
        h.slice_start = start;
        h.slice_end = end;
        h.slice_duration = duration;
        h.dim_info = dim_info;
        h.xyzt_units = 10;
        h
    }

    #[test]
    fn header_timing_follows_observed_rules() {
        let times = |code| {
            header_timing(&header(code, 0, 5, 0.25, 48), 6)
                .unwrap()
                .times
        };
        assert_eq!(times(1), [0.0, 0.25, 0.5, 0.75, 1.0, 1.25]);
        assert_eq!(times(2), [0.0; 6]);
        assert_eq!(times(3), [0.0, 0.75, 0.25, 1.0, 0.5, 1.25]);
        assert_eq!(times(4), [1.25, 0.5, 1.0, 0.25, 0.75, 0.0]);
        assert_eq!(times(5), [0.75, 0.0, 1.0, 0.25, 1.25, 0.5]);
        assert_eq!(times(6), [0.5, 1.25, 0.25, 1.0, 0.0, 0.75]);
        assert_eq!(times(9), [0.0; 6]);
        let partial = header_timing(&header(3, 1, 4, 0.25, 48), 6).unwrap();
        assert_eq!(partial.times, [0.0, 0.0, 0.5, 0.25, 0.75, 0.0]);
        // No timing: wrong slice axis, no duration, bad range, non-positive code.
        assert!(header_timing(&header(3, 0, 5, 0.25, 16), 6).is_none());
        assert!(header_timing(&header(3, 0, 5, 0.0, 48), 6).is_none());
        assert!(header_timing(&header(3, 0, 0, 0.25, 48), 6).is_none());
        assert!(header_timing(&header(3, 0, 6, 0.25, 48), 6).is_none());
        assert!(header_timing(&header(200, 0, 5, 0.25, 48), 6).is_none());
        assert!(header_timing(&header(0, 0, 5, 0.25, 48), 6).is_none());
        // Milliseconds are converted.
        let mut ms = header(3, 0, 5, 250.0, 48);
        ms.xyzt_units = 2 | 16;
        assert_eq!(header_timing(&ms, 6).unwrap().duration, 0.25);
    }

    #[test]
    fn mean_is_a_float32_mean() {
        let times = Pattern::AltMinus.times(6, 2.0);
        let mut sum = 0f32;
        for t in &times {
            sum += t;
        }
        assert_eq!(mean_time(&times).to_bits(), (sum / 6.0).to_bits());
    }
}
