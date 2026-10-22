// SPDX-License-Identifier: Apache-2.0
//! Atropos' outlier handling (`--winsorize-outliers`): ANTs' `BoxPlotQuantileListSampleFilter`
//! and `GrubbsRosnerListSampleFilter` (ANTs v2.6.5, `ImageSegmentation/`), with ITK v5.4.5's
//! `SampleToHistogramFilter`, `Histogram::Quantile` and `TDistribution`.
//!
//! **What ANTs really computes (ANTS-94).** Atropos keeps one filter object for every image and
//! iteration, and neither filter clears its output list before it runs:
//! - GrubbsRosner appends each call's output to the list, so Atropos' samples grow by the earlier
//!   calls' values and the second and later images read the first image's;
//! - BoxPlot computes its replacement values from a histogram of its own output list, through a
//!   second ITK filter whose update runs BoxPlot once more inside itself (verified with a probe
//!   built on the oracle's ITK): the inner run pushes the non-outliers again and winsorizes with
//!   the bounds of `previous list + non-outliers + non-outliers`, and the outer run takes its
//!   replacement values from a histogram of that inner result.
//!
//! larmorx's default is the documented filter: the replacement values are the whisker bounds of
//! the non-outliers' histogram, and each call starts from an empty list.

/// The filter chosen with `-w`, with its parameters (as `double`, the filters' `RealType`;
/// ANTs reads them as `float`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OutlierHandling {
    /// `BoxPlot[lowerPercentile, upperPercentile, whiskerLength]` (0.25, 0.75, 1.5).
    BoxPlot { lower: f64, upper: f64, whisker: f64 },
    /// `GrubbsRosner[significanceLevel, winsorizingLevel]` (0.05, 0.10).
    GrubbsRosner { significance: f64, winsorizing: f64 },
}

/// The filter object Atropos keeps, with the output list ANTs' filter would hold.
#[derive(Clone, Debug)]
pub struct OutlierFilter {
    pub handling: OutlierHandling,
    /// ANTs' output list after the previous call.
    ants_state: Vec<f32>,
    /// Use the documented filter (larmorx's correction of ANTS-94); `false` reproduces ANTs.
    pub fix: bool,
}

/// Why the filter failed (an ITK exception).
#[derive(Clone, Debug, PartialEq)]
pub struct OutlierError(pub String);

impl OutlierFilter {
    pub fn new(handling: OutlierHandling, fix: bool) -> Self {
        OutlierFilter {
            handling,
            ants_state: Vec::new(),
            fix,
        }
    }

    /// `SetInputListSample(input); Update(); GetOutput()`: the output list Atropos reads, and
    /// whether ANTs' list differs from the documented filter's (ANTS-94).
    pub fn apply(&mut self, input: &[f32]) -> Result<(Vec<f32>, bool), OutlierError> {
        let (clean, ants) = match self.handling {
            OutlierHandling::BoxPlot {
                lower,
                upper,
                whisker,
            } => {
                let clean = box_plot_documented(input, lower, upper, whisker)?;
                let ants = box_plot_ants(input, lower, upper, whisker, &self.ants_state)?;
                (clean, ants)
            }
            OutlierHandling::GrubbsRosner {
                significance,
                winsorizing,
            } => {
                let clean = grubbs_rosner(input, significance, winsorizing)?;
                let mut ants = self.ants_state.clone();
                ants.extend_from_slice(&clean);
                (clean, ants)
            }
        };
        let changed = clean != ants;
        self.ants_state = ants.clone();
        Ok((if self.fix { clean } else { ants }, changed))
    }
}

/// `SampleToHistogramFilter` with 200 bins and automatic bounds (marginal scale 100), then
/// `Histogram::Quantile(0, p)` at both percentiles.
fn quantiles(values: &[f32], lower: f64, upper: f64) -> Result<(f64, f64), OutlierError> {
    let h = larmorx_image::statistics::scalar_image_histogram(values, 200, 1)
        .map_err(|e| OutlierError(e.to_string()))?;
    Ok((h.quantile(lower), h.quantile(upper)))
}

/// The whisker bounds `(q_l − w·(q_u − q_l), q_u + w·(q_u − q_l))`.
fn whiskers(values: &[f32], lower: f64, upper: f64, whisker: f64) -> Result<(f64, f64), OutlierError> {
    let (lq, uq) = quantiles(values, lower, upper)?;
    Ok((lq - whisker * (uq - lq), uq + whisker * (uq - lq)))
}

fn check(lower: f64, upper: f64) -> Result<(), OutlierError> {
    if lower >= upper {
        return Err(OutlierError(
            "Lower percentile must be less than upper percentile.".into(),
        ));
    }
    Ok(())
}

/// The values of `input` within the whisker bounds `(lb, ub)`.
fn kept(input: &[f32], lb: f64, ub: f64) -> Vec<f32> {
    input
        .iter()
        .copied()
        .filter(|&x| {
            let v = f64::from(x);
            !(v < lb || v > ub)
        })
        .collect()
}

/// The winsorized list: values below `lb` become `lb2`, values above `ub` become `ub2`.
fn winsorize(input: &[f32], lb: f64, ub: f64, lb2: f64, ub2: f64) -> Vec<f32> {
    input
        .iter()
        .map(|&x| {
            let v = f64::from(x);
            if v < lb {
                lb2 as f32
            } else if v > ub {
                ub2 as f32
            } else {
                x
            }
        })
        .collect()
}

/// The documented BoxPlot winsorizing: outliers beyond the first histogram's whiskers are
/// replaced by the whiskers of the non-outliers' histogram.
fn box_plot_documented(
    input: &[f32],
    lower: f64,
    upper: f64,
    whisker: f64,
) -> Result<Vec<f32>, OutlierError> {
    check(lower, upper)?;
    let (lb, ub) = whiskers(input, lower, upper, whisker)?;
    let (lb2, ub2) = whiskers(&kept(input, lb, ub), lower, upper, whisker)?;
    Ok(winsorize(input, lb, ub, lb2, ub2))
}

/// ANTs' BoxPlot call on a list holding `previous` (see the module): the inner run's
/// replacement bounds come from `previous + kept + kept`, the outer run's from the inner
/// result.
fn box_plot_ants(
    input: &[f32],
    lower: f64,
    upper: f64,
    whisker: f64,
    previous: &[f32],
) -> Result<Vec<f32>, OutlierError> {
    check(lower, upper)?;
    let (lb, ub) = whiskers(input, lower, upper, whisker)?;
    let k = kept(input, lb, ub);
    let mut inner_list = Vec::with_capacity(previous.len() + 2 * k.len());
    inner_list.extend_from_slice(previous);
    inner_list.extend_from_slice(&k);
    inner_list.extend_from_slice(&k);
    let (lb2a, ub2a) = whiskers(&inner_list, lower, upper, whisker)?;
    let inner = winsorize(input, lb, ub, lb2a, ub2a);
    let (lb2b, ub2b) = whiskers(&inner, lower, upper, whisker)?;
    Ok(winsorize(input, lb, ub, lb2b, ub2b))
}

/// `GrubbsRosnerListSampleFilter::GenerateData` (with winsorizing) on a cleared list: the
/// value farthest from the mean is an outlier while Grubbs' statistic exceeds its critical
/// value (the mean and variance updated after each removal, as ANTs updates them), as long as
/// more than 6 values remain; outliers become the mean ± the Student quantile of the
/// winsorizing level times the standard deviation.
fn grubbs_rosner(input: &[f32], significance: f64, winsorizing: f64) -> Result<Vec<f32>, OutlierError> {
    let n = input.len();
    if n <= 6 {
        return Ok(input.to_vec());
    }
    let mut mean = 0.0f64;
    let mut variance = 0.0f64;
    let mut count = 0.0f64;
    for &x in input {
        let x = f64::from(x);
        count += 1.0;
        variance += (count - 1.0) * ((x - mean) * (x - mean)) / count;
        mean += (x - mean) / count;
    }
    variance /= count - 1.0;
    let mut slatec = super::tdist::Slatec::default();
    let mut is_outlier = vec![false; n];
    let mut outliers = 0usize;
    let mut found = true;
    while found && n - outliers > 6 {
        found = false;
        // FindMaximumNonOutlierDeviationValue
        let mut max_dev = 0.0f64;
        let mut max_id: Option<usize> = None;
        for (id, &x) in input.iter().enumerate() {
            if !is_outlier[id] {
                let dev = (f64::from(x) - mean).abs();
                if dev > max_dev {
                    max_dev = dev;
                    max_id = Some(id);
                }
            }
        }
        if let Some(id) = max_id {
            let x = f64::from(input[id]);
            let remaining = (n - outliers) as u64;
            // IsMeasurementAnOutlier
            let sig = significance / (2.0 * remaining as f64);
            let t = slatec.t_inverse_cdf(1.0 - sig, remaining - 2);
            let nu = (remaining - 1) as f64;
            let g = nu / (nu + 1.0).sqrt() * (t * t / (nu - 1.0 + t * t)).sqrt();
            found = g < (x - mean).abs() / variance.sqrt();
            if found {
                let count2 = remaining as f64;
                mean = (mean * count2 - x) / (count2 - 1.0);
                variance = (count2 - 1.0) * variance - (count2 - 1.0) * ((x - mean) * (x - mean)) / count2;
                variance /= count2 - 2.0;
                is_outlier[id] = true;
                outliers += 1;
            }
        }
    }
    let t = slatec.t_inverse_cdf(1.0 - 0.5 * winsorizing, (n - outliers) as u64);
    let lower = mean - t * variance.sqrt();
    let upper = mean + t * variance.sqrt();
    Ok(input
        .iter()
        .enumerate()
        .map(|(id, &x)| {
            if !is_outlier[id] {
                x
            } else if f64::from(x) < lower {
                lower as f32
            } else {
                upper as f32
            }
        })
        .collect())
}

/// `Print(os, indent)` of the outlier filter: the header and its `PrintSelf`.
pub fn print(h: &OutlierHandling, indent: &str) -> String {
    let next = format!("{indent}  ");
    let g = |v: f64| crate::cli::cstd::format_g(v);
    match *h {
        OutlierHandling::BoxPlot {
            lower,
            upper,
            whisker,
        } => format!(
            "{indent}BoxPlotQuantileListSampleFilter (0x0)\n{next}Percentile Bounds: [{}, {}]\n{next}Whisker scaling factor: {}\n{next}Outlier handling: Winsorize\n",
            g(lower),
            g(upper),
            g(whisker)
        ),
        OutlierHandling::GrubbsRosner {
            significance,
            winsorizing,
        } => format!(
            "{indent}GrubbsRosnerListSampleFilter (0x0)\n{next}Significance level: {}\n{next}Outlier handling: Winsorize (level = {})\n",
            g(significance),
            g(winsorizing)
        ),
    }
}
