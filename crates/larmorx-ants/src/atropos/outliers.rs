// SPDX-License-Identifier: Apache-2.0
//! Atropos' outlier handling (`--winsorize-outliers`): ANTs' `BoxPlotQuantileListSampleFilter`
//! and `GrubbsRosnerListSampleFilter` (ANTs v2.6.5, `ImageSegmentation/`).

/// The filter chosen with `-w`, with its parameters (as `double`, the filters' `RealType`;
/// ANTs reads them as `float`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OutlierHandling {
    /// `BoxPlot[lowerPercentile, upperPercentile, whiskerLength]` (0.25, 0.75, 1.5).
    BoxPlot { lower: f64, upper: f64, whisker: f64 },
    /// `GrubbsRosner[significanceLevel, winsorizingLevel]` (0.05, 0.10).
    GrubbsRosner { significance: f64, winsorizing: f64 },
}

/// A filter object, with the output ANTs never clears between calls (ANTS-94).
#[derive(Clone, Debug)]
pub struct OutlierFilter {
    pub handling: OutlierHandling,
    /// The output list sample as it stands after the previous call.
    output: Vec<f32>,
    /// Clear the output before each call (larmorx's correction of ANTS-94).
    pub clear: bool,
}

/// Why the filter failed (an ITK exception).
#[derive(Clone, Debug, PartialEq)]
pub struct OutlierError(pub String);

impl OutlierFilter {
    pub fn new(handling: OutlierHandling, clear: bool) -> Self {
        OutlierFilter {
            handling,
            output: Vec::new(),
            clear,
        }
    }

    /// `SetInputListSample(input); Update(); GetOutput()`: the output list (its first
    /// `input.len()` values are what Atropos copies).
    pub fn apply(&mut self, input: &[f32]) -> Result<&[f32], OutlierError> {
        if self.clear {
            self.output.clear();
        }
        match self.handling {
            OutlierHandling::BoxPlot {
                lower,
                upper,
                whisker,
            } => box_plot(input, lower, upper, whisker, &mut self.output)?,
            OutlierHandling::GrubbsRosner {
                significance,
                winsorizing,
            } => grubbs_rosner(input, significance, winsorizing, &mut self.output)?,
        }
        Ok(&self.output)
    }
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

fn box_plot(
    _input: &[f32],
    _lower: f64,
    _upper: f64,
    _whisker: f64,
    _output: &mut Vec<f32>,
) -> Result<(), OutlierError> {
    Err(OutlierError("BoxPlot is not implemented yet".into()))
}

fn grubbs_rosner(
    _input: &[f32],
    _significance: f64,
    _winsorizing: f64,
    _output: &mut Vec<f32>,
) -> Result<(), OutlierError> {
    Err(OutlierError("GrubbsRosner is not implemented yet".into()))
}
