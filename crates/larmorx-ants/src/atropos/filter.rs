// SPDX-License-Identifier: Apache-2.0
//! `AtroposSegmentationImageFilter` (ANTs v2.6.5,
//! `ImageSegmentation/antsAtroposSegmentationImageFilter.hxx`), step by step.
//!
//! Images are flat slices in Fortran order (ITK's raster order: the first axis fastest), on
//! one grid of `size` and `spacing`. Labels are `u32` (`Image<unsigned int, D>`), every other
//! image `f32`, as Atropos declares them; the arithmetic of each step is in the types ANTs
//! uses (`float` for the posteriors, MRF and priors; `double` for the Gaussian models and the
//! sums ITK accumulates in `double`).

use std::collections::BTreeMap;

use larmorx_core::defects::{self, Notices};
use larmorx_core::math::{expf, powf};
use larmorx_image::itk_math::almost_equal_f32;
use larmorx_image::kdtree::{KdTree, Kmeans};
use larmorx_registration::rng::MersenneTwister;
use rayon::prelude::*;

use super::fast_marching::fast_marching;
use super::gaussian::EstimateError;
use super::models::Likelihood;
use super::outliers::{OutlierError, OutlierFilter};
use super::smoothing::ControlLattice;
use super::{AtroposOptions, Formulation, Initialization};

const HXX: &str = "ImageSegmentation/antsAtroposSegmentationImageFilter.hxx";

/// An ITK exception thrown inside the filter's `Update` (Atropos.cxx catches it: with `-v` it
/// prints it, and exits 1), or a case larmorx refuses (ANTs' behaviour is undefined there).
#[derive(Clone, Debug, PartialEq)]
pub enum AtroposError {
    /// `itk::ExceptionObject`: the file, line, class and description.
    Itk {
        file: &'static str,
        line: u32,
        class: &'static str,
        description: String,
    },
    /// ANTs' behaviour is undefined (a crash, a read past the data): larmorx refuses.
    Refused(String),
}

impl std::fmt::Display for AtroposError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtroposError::Itk { description, .. } => f.write_str(description),
            AtroposError::Refused(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for AtroposError {}

impl From<EstimateError> for AtroposError {
    fn from(e: EstimateError) -> Self {
        AtroposError::Itk {
            file: e.file,
            line: e.line,
            class: e.class,
            description: e.description,
        }
    }
}

impl From<OutlierError> for AtroposError {
    fn from(e: OutlierError) -> Self {
        AtroposError::Itk {
            file: "ImageSegmentation/antsBoxPlotQuantileListSampleFilter.hxx",
            line: 0,
            class: "ListSampleToListSampleFilter",
            description: e.0,
        }
    }
}

fn itk(line: u32, description: impl Into<String>) -> AtroposError {
    AtroposError::Itk {
        file: HXX,
        line,
        class: "AtroposSegmentationImageFilter",
        description: description.into(),
    }
}

/// One iteration as the verbose observer reports it: the elapsed iterations and the posterior
/// probability at the start of the iteration (`IterationEvent` fires before the update).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IterationRecord {
    pub elapsed: u32,
    pub posterior_probability: f32,
    pub annealing_temperature: f32,
}

/// `GetIntegerVariate(n)`: a value in `[0, n]` by rejection on the smallest mask of ones.
#[inline]
fn integer_variate(rng: &mut MersenneTwister, n: u32) -> u32 {
    let mut used = n;
    used |= used >> 1;
    used |= used >> 2;
    used |= used >> 4;
    used |= used >> 8;
    used |= used >> 16;
    loop {
        let i = rng.integer() & used;
        if i <= n {
            return i;
        }
    }
}

/// `std::max(a, b)` for floats: `a < b ? b : a`.
#[inline]
fn max_f(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// The neighbourhood of an MRF radius: offsets in ITK's order (first axis fastest).
#[derive(Clone, Debug)]
struct Neighborhood {
    /// Per element, the offset along each axis.
    offsets: Vec<Vec<i64>>,
    /// Per element, the linear offset.
    linear: Vec<i64>,
    center: usize,
}

impl Neighborhood {
    fn new(radius: &[u32], strides: &[usize]) -> Self {
        let d = radius.len();
        let sizes: Vec<usize> = radius.iter().map(|&r| 2 * r as usize + 1).collect();
        let n: usize = sizes.iter().product();
        let mut offsets = Vec::with_capacity(n);
        let mut linear = Vec::with_capacity(n);
        for k in 0..n {
            let mut rest = k;
            let mut o = vec![0i64; d];
            let mut lin = 0i64;
            for a in 0..d {
                o[a] = (rest % sizes[a]) as i64 - i64::from(radius[a]);
                rest /= sizes[a];
                lin += o[a] * strides[a] as i64;
            }
            offsets.push(o);
            linear.push(lin);
        }
        Neighborhood {
            offsets,
            linear,
            center: n / 2,
        }
    }

    fn len(&self) -> usize {
        self.linear.len()
    }
}

/// The filter.
pub struct Atropos<'a> {
    pub options: &'a AtroposOptions,
    size: Vec<usize>,
    spacing: Vec<f64>,
    origin: Vec<f64>,
    strides: Vec<usize>,
    n: usize,
    intensities: Vec<&'a [f32]>,
    mask: Option<&'a [u32]>,
    n_threads: usize,
    /// The output labels (`GetOutput()`).
    pub labels: Vec<u32>,
    rng: MersenneTwister,
    components: Vec<Likelihood>,
    pub proportions: Vec<f32>,
    label_volumes: Vec<u64>,
    /// Dense prior probability images (non-minimized) or the sparse ones' values.
    priors: Vec<Vec<f32>>,
    sparse_priors: Vec<Vec<(u64, f32)>>,
    prior_label: Option<Vec<u32>>,
    distance_priors: Vec<Vec<f32>>,
    sum_distance_prior: Vec<f32>,
    posteriors: Vec<Vec<f32>>,
    sum_posterior: Vec<f32>,
    icm_codes: Option<Vec<u32>>,
    pub maximum_icm_code: u32,
    lattices: Vec<Vec<Option<ControlLattice>>>,
    pub elapsed_iterations: u32,
    pub current_posterior_probability: f32,
    inverse_distances: Vec<f32>,
    delta: Vec<Vec<f32>>,
    neighborhood: Neighborhood,
    outlier_filter: Option<OutlierFilter>,
    pub notices: Notices,
    /// ITK warnings (`itkWarningMacro`), in order.
    pub warnings: Vec<String>,
    pub iterations: Vec<IterationRecord>,
}

impl<'a> Atropos<'a> {
    /// The filter on `intensities` (at least one; all on the grid of `size` and `spacing`),
    /// with an optional `mask` and the options' priors.
    pub fn new(
        options: &'a AtroposOptions,
        size: &[usize],
        spacing: &[f64],
        origin: &[f64],
        intensities: Vec<&'a [f32]>,
        mask: Option<&'a [u32]>,
        n_threads: usize,
    ) -> Result<Self, AtroposError> {
        let d = size.len();
        let n: usize = size.iter().product();
        let mut strides = vec![1usize; d];
        for a in 1..d {
            strides[a] = strides[a - 1] * size[a - 1];
        }
        if intensities.is_empty() || intensities.iter().any(|i| i.len() != n) {
            return Err(AtroposError::Refused(
                "the intensity images must have the first image's size".into(),
            ));
        }
        let classes = options.classes.max(2) as usize;
        let total = classes + options.partial_volume_classes();
        let mut components = Vec::with_capacity(total);
        for _ in 0..classes {
            components.push(Likelihood::new(&options.likelihood, options.upstream_defects));
        }
        for _ in classes..total {
            components.push(Likelihood::PartialVolume(Default::default()));
        }
        let neighborhood = Neighborhood::new(&options.mrf_radius, &strides);
        let outlier_filter = options
            .outliers
            .map(|h| OutlierFilter::new(h, options.upstream_defects.fix()));
        let mut filter = Atropos {
            options,
            size: size.to_vec(),
            spacing: spacing.to_vec(),
            origin: origin.to_vec(),
            strides,
            n,
            intensities,
            mask,
            n_threads,
            labels: vec![0; n],
            rng: MersenneTwister::new(options.seed),
            components,
            proportions: vec![0.0; total],
            label_volumes: vec![0; total],
            priors: Vec::new(),
            sparse_priors: Vec::new(),
            prior_label: None,
            distance_priors: Vec::new(),
            sum_distance_prior: Vec::new(),
            posteriors: Vec::new(),
            sum_posterior: Vec::new(),
            icm_codes: None,
            maximum_icm_code: 0,
            lattices: Vec::new(),
            elapsed_iterations: 0,
            current_posterior_probability: 0.0,
            inverse_distances: Vec::new(),
            delta: Vec::new(),
            neighborhood,
            outlier_filter,
            notices: Notices::new(options.upstream_defects),
            warnings: Vec::new(),
            iterations: Vec::new(),
        };
        match &options.initialization {
            Initialization::PriorProbabilityImages { images } => {
                for (k, image) in images.iter().enumerate() {
                    if image.len() != n {
                        return Err(AtroposError::Refused(format!(
                            "prior probability image {} has {} voxels, not {n}",
                            k + 1,
                            image.len()
                        )));
                    }
                    filter.set_prior(k, image.clone());
                }
            }
            Initialization::PriorLabelImage { image } => {
                if image.len() != n {
                    return Err(AtroposError::Refused(format!(
                        "the prior label image has {} voxels, not {n}",
                        image.len()
                    )));
                }
                filter.prior_label = Some(image.clone());
            }
            _ => {}
        }
        Ok(filter)
    }

    fn classes(&self) -> usize {
        self.options.classes.max(2) as usize
    }

    fn total_classes(&self) -> usize {
        self.classes() + self.options.partial_volume_classes()
    }

    #[inline]
    fn in_mask(&self, v: usize) -> bool {
        match self.mask {
            Some(m) => m[v] != 0,
            None => true,
        }
    }

    fn uses_prior_probabilities(&self) -> bool {
        matches!(
            self.options.initialization,
            Initialization::PriorProbabilityImages { .. }
        )
    }

    fn uses_prior_label(&self) -> bool {
        matches!(self.options.initialization, Initialization::PriorLabelImage { .. })
    }

    fn uses_priors(&self) -> bool {
        self.options.initialization.uses_priors()
    }

    // ---------------------------------------------------------------------------------------
    // Prior probability images

    /// `SetPriorProbabilityImage(k + 1, image)`: dense, or sparse with `--minimize-memory-usage`
    /// (the values above the probability threshold, at their voxel numbers; ANTs stores the
    /// numbers in a float point set, ANTS-97).
    /// Where class `k`'s prior is stored (one slot for all with aliased priors).
    fn slot(&self, k: usize) -> usize {
        if self.options.aliased_priors { 0 } else { k }
    }

    fn set_prior(&mut self, k: usize, image: Vec<f32>) {
        let k = self.slot(k);
        if self.options.minimize_memory {
            let threshold = self.options.probability_threshold;
            let reproduce = self.options.upstream_defects.reproduce();
            let mut sparse = Vec::new();
            let mut rounded = false;
            for (v, &p) in image.iter().enumerate() {
                if p > threshold {
                    let number = if reproduce {
                        let f = v as u64 as f32;
                        let r = f as u64;
                        rounded |= r != v as u64;
                        r
                    } else {
                        rounded |= (v as u64 as f32) as u64 != v as u64;
                        v as u64
                    };
                    sparse.push((number, p));
                }
            }
            if rounded {
                self.notices.add(
                    &defects::ANTS_97,
                    "voxel numbers above 2^24 of the sparse prior probability images",
                );
            }
            if self.sparse_priors.len() <= k {
                self.sparse_priors.resize(k + 1, Vec::new());
            }
            self.sparse_priors[k] = sparse;
        } else {
            if self.priors.len() <= k {
                self.priors.resize(k + 1, Vec::new());
            }
            self.priors[k] = image;
        }
    }

    /// `GetPriorProbabilityImage(c + 1)` (`None` without prior probability images or for a
    /// partial-volume class).
    fn prior(&self, c: usize) -> Option<std::borrow::Cow<'_, [f32]>> {
        if !self.uses_prior_probabilities() || c >= self.classes() {
            return None;
        }
        if self.options.minimize_memory {
            let mut out = vec![0.0f32; self.n];
            for &(number, p) in &self.sparse_priors[self.slot(c)] {
                if let Some(slot) = out.get_mut(number as usize) {
                    *slot = p;
                }
            }
            Some(std::borrow::Cow::Owned(out))
        } else {
            Some(std::borrow::Cow::Borrowed(&self.priors[self.slot(c)]))
        }
    }

    // ---------------------------------------------------------------------------------------
    // GenerateData

    /// `GenerateData`: the initial labelling and model, then the iterations until convergence.
    pub fn run(&mut self) -> Result<(), AtroposError> {
        self.generate_initial_class_labeling()?;
        self.compute_mrf_neighborhood();
        let mut converged = false;
        self.current_posterior_probability = 0.0;
        let mut old = -f32::MAX;
        let mut iteration = 0u32;
        while !converged && iteration < self.options.max_iterations {
            iteration += 1;
            self.iterations.push(IterationRecord {
                elapsed: self.elapsed_iterations,
                posterior_probability: self.current_posterior_probability,
                annealing_temperature: self.annealing_temperature(),
            });
            self.current_posterior_probability = self.update_class_labeling()?;
            if self.current_posterior_probability - old < self.options.convergence_threshold {
                converged = true;
            }
            old = self.current_posterior_probability;
            self.elapsed_iterations += 1;
            self.posteriors.clear();
        }
        Ok(())
    }

    /// The annealing temperature of the current iteration, as the verbose observer computes
    /// it: `T0 · rate^elapsed`, at least the minimum.
    pub fn annealing_temperature(&self) -> f32 {
        let o = self.options;
        let t = o.initial_annealing_temperature
            * powf(o.annealing_rate, self.elapsed_iterations as f32);
        max_f(t, o.minimum_annealing_temperature)
    }

    /// `ComputeMRFNeighborhoodDistances` and `ComputeMRFDeltaMatrix`.
    fn compute_mrf_neighborhood(&mut self) {
        let nb = &self.neighborhood;
        let mut inv = vec![0.0f32; nb.len()];
        for (k, slot) in inv.iter_mut().enumerate() {
            if k == nb.center {
                *slot = 0.0;
            } else {
                let mut distance = 0.0f32;
                for (a, &o) in nb.offsets[k].iter().enumerate() {
                    let phys = o as f32 * self.spacing[a] as f32;
                    distance += phys * phys;
                }
                *slot = (1.0 / f64::from(distance.sqrt())) as f32;
            }
        }
        self.inverse_distances = inv;
        let total = self.total_classes();
        let has_pv = total > self.classes();
        let mut delta = vec![vec![0.0f32; total]; total];
        for (l, row) in delta.iter_mut().enumerate() {
            for (k, slot) in row.iter_mut().enumerate() {
                let (label, neighbour) = (l as u32 + 1, k as u32 + 1);
                *slot = if label == neighbour {
                    if has_pv { -2.0 } else { -1.0 }
                } else if has_pv {
                    let common = self
                        .options
                        .partial_volume_sets
                        .iter()
                        .any(|s| s.contains(&label) && s.contains(&neighbour));
                    if common { -1.0 } else { 1.0 }
                } else {
                    0.0
                };
            }
        }
        self.delta = delta;
    }

    /// `EvaluateMRFNeighborhoodWeights` at voxel `v` with `labels`.
    #[inline]
    fn mrf_weights(&self, labels: &[u32], v: usize, coord: &[usize], out: &mut [f32], h: &mut [f32]) {
        let total = out.len();
        h.iter_mut().for_each(|x| *x = 0.0);
        let nb = &self.neighborhood;
        'elements: for k in 0..nb.len() {
            if k == nb.center {
                continue;
            }
            for (a, &o) in nb.offsets[k].iter().enumerate() {
                let c = coord[a] as i64 + o;
                if c < 0 || c >= self.size[a] as i64 {
                    continue 'elements;
                }
            }
            let label = labels[(v as i64 + nb.linear[k]) as usize];
            if label == 0 {
                continue;
            }
            let inv = self.inverse_distances[k];
            if inv <= 0.0 {
                continue;
            }
            // ITK indexes H[label - 1] without a check: a label above the classes (a prior
            // label image with more labels) reads past H; larmorx skips it.
            if let Some(slot) = h.get_mut(label as usize - 1) {
                *slot += inv;
            }
        }
        for l in 0..total {
            let mut sum = 0.0f32;
            for k in 0..total {
                sum += self.delta[l][k] * h[k];
            }
            out[l] = sum;
        }
    }

    fn coord(&self, v: usize, out: &mut [usize]) {
        for a in 0..self.size.len() {
            out[a] = (v / self.strides[a]) % self.size[a];
        }
    }

    // ---------------------------------------------------------------------------------------
    // Initial labelling

    /// `GenerateInitialClassLabeling`.
    fn generate_initial_class_labeling(&mut self) -> Result<(), AtroposError> {
        self.labels = vec![0; self.n];
        match &self.options.initialization {
            Initialization::Random => {
                let k = self.classes() as u32;
                let fix = self.options.upstream_defects.fix();
                let mut outside = false;
                for v in 0..self.n {
                    let label = integer_variate(&mut self.rng, k - 1) + 1;
                    if self.in_mask(v) || !fix {
                        self.labels[v] = label;
                    }
                    outside |= !self.in_mask(v);
                }
                if outside {
                    self.notices.add(
                        &defects::ANTS_92,
                        "the voxels outside the mask were labelled and entered the first estimates",
                    );
                }
            }
            Initialization::KMeans { .. } => self.kmeans_labeling()?,
            Initialization::Otsu => self.otsu_labeling()?,
            Initialization::PriorProbabilityImages { .. } => self.prior_probability_labeling()?,
            Initialization::PriorLabelImage { .. } => {
                let prior = self.prior_label.as_ref().expect("prior label image");
                for v in 0..self.n {
                    self.labels[v] = if self.in_mask(v) { prior[v] } else { 0 };
                }
            }
        }
        let classes = self.classes();
        let total = self.total_classes();
        let m = self.intensities.len();
        // The samples of each label (labels 0 skipped; labels beyond the classes would index
        // past ANTs' vector of samples).
        let mut samples: Vec<Vec<f32>> = vec![Vec::new(); total];
        for v in 0..self.n {
            let label = self.labels[v] as usize;
            if label == 0 {
                continue;
            }
            let Some(sample) = samples.get_mut(label - 1) else {
                return Err(AtroposError::Refused(format!(
                    "the initial labelling has label {label}, above the {total} classes \
                     (ANTs writes past its list of samples)"
                )));
            };
            for img in &self.intensities {
                sample.push(img[v]);
            }
        }
        let sizes: Vec<usize> = samples.iter().map(|s| s.len() / m).collect();
        let total_size: usize = sizes.iter().sum();
        let mut weights: Vec<Vec<f64>> = sizes.iter().map(|&s| vec![1.0f64; s]).collect();
        for (c, &s) in sizes.iter().enumerate() {
            self.label_volumes[c] = s as u64;
        }
        if self.uses_prior_probabilities() {
            let dense: Vec<Vec<f32>> = (0..classes)
                .map(|c| self.prior(c).expect("prior").into_owned())
                .collect();
            let mut count = vec![0usize; total];
            for v in 0..self.n {
                let label = self.labels[v] as usize;
                if label == 0 || label > classes {
                    continue;
                }
                weights[label - 1][count[label - 1]] = f64::from(dense[label - 1][v]);
                count[label - 1] += 1;
            }
        }
        for c in 0..total {
            if c < classes {
                let w = self.components[c].set_sample(&samples[c], m, &weights[c])?;
                self.model_notices(c, &weights[c]);
                if let Some(w) = w {
                    self.warnings.push(w.to_string());
                }
                self.proportions[c] = if self.options.use_mixture_model_proportions {
                    sizes[c] as f32 / total_size as f32
                } else {
                    1.0 / total as f32
                };
            } else {
                let set = self.options.partial_volume_sets[c - classes].clone();
                for (d, &l) in set.iter().enumerate() {
                    let l = l as usize - 1;
                    let w = self.components[c].set_indexed_sample(d, &samples[l], m, &weights[l])?;
                    if let Some(w) = w {
                        self.warnings.push(w.to_string());
                    }
                }
                self.proportions[c] = 0.0;
            }
        }
        self.lattices = vec![vec![None; classes]; m];
        Ok(())
    }

    /// `GenerateInitialClassLabelingWithKMeansClustering`.
    fn kmeans_labeling(&mut self) -> Result<(), AtroposError> {
        let classes = self.classes();
        let fix = self.options.upstream_defects.fix();
        let first = self.intensities[0];
        // LabelStatisticsImageFilter on the mask's labels (or ones): the range of label 1.
        let mut min = f64::from(f32::MAX);
        let mut max = -f64::from(f32::MAX);
        let mut found_one = false;
        let mut range_any = (f64::from(f32::MAX), -f64::from(f32::MAX));
        for v in 0..self.n {
            let label = self.mask.map_or(1, |m| m[v]);
            let x = f64::from(first[v]);
            if label == 1 {
                found_one = true;
                if x < min {
                    min = x;
                }
                if x > max {
                    max = x;
                }
            }
            if label != 0 {
                if x < range_any.0 {
                    range_any.0 = x;
                }
                if x > range_any.1 {
                    range_any.1 = x;
                }
            }
        }
        let (mut min_value, mut max_value) = (min as f32, max as f32);
        let differs = !found_one || (min, max) != range_any;
        if differs && self.mask.is_some() {
            // ANTS-98: the range of the voxels labelled 1, not of the mask.
            if fix {
                min_value = range_any.0 as f32;
                max_value = range_any.1 as f32;
            }
            self.notices.add(
                &defects::ANTS_98,
                "the first image's range taken from the voxels labelled 1 in the mask",
            );
        }
        let mut sample = Vec::new();
        let mut sample_voxels = Vec::new();
        for v in 0..self.n {
            if self.in_mask(v) {
                sample.push(first[v]);
                sample_voxels.push(v);
            }
        }
        let initial: Vec<f64> = match &self.options.initialization {
            Initialization::KMeans {
                centers: Some(centers),
            } if centers.len() == classes => centers.iter().map(|&c| f64::from(c)).collect(),
            _ => (0..classes)
                .map(|k| {
                    f64::from(
                        min_value
                            + (max_value - min_value) * (k as f32 + 0.5) / classes as f32,
                    )
                })
                .collect(),
        };
        let centers = self.kmeans(&sample, 1, &initial)?;
        let mut sorted: Vec<f32> = centers.iter().map(|&c| c as f32).collect();
        sorted.sort_by(|a, b| a.total_cmp(b));
        // SampleClassifierFilter with MinimumDecisionRule: the first nearest centre.
        let labels1: Vec<u32> = sample
            .par_iter()
            .map(|&x| {
                let mut best = 0usize;
                let mut min = 0.0f64;
                for (k, &c) in sorted.iter().enumerate() {
                    let d = (f64::from(c) - f64::from(x)).abs();
                    if k == 0 || d < min {
                        min = d;
                        best = k;
                    }
                }
                best as u32 + 1
            })
            .collect();
        for (i, &v) in sample_voxels.iter().enumerate() {
            self.labels[v] = labels1[i];
        }
        let m = self.intensities.len();
        if m > 1 {
            // The second k-means, on every image. ANTs seeds it with the mask's statistics
            // (`stats->GetMean(j + 1)` of the first image) and then relabels with the first
            // classifier's output, so it changes nothing (ANTS-90). larmorx computes it as
            // intended: seeded with each class's mean of each image in the first labelling,
            // its labels kept; reproduced, the first labelling stays.
            let mut initial2 = vec![0.0f64; classes * m];
            for i in 0..m {
                let mut sums = vec![0.0f64; classes];
                let mut counts = vec![0u64; classes];
                for v in 0..self.n {
                    let l = self.labels[v] as usize;
                    if l >= 1 && l <= classes {
                        sums[l - 1] += f64::from(self.intensities[i][v]);
                        counts[l - 1] += 1;
                    }
                }
                for k in 0..classes {
                    // classMeanValues is a VariableSizeMatrix<float>.
                    let mean = if counts[k] > 0 {
                        sums[k] / counts[k] as f64
                    } else {
                        0.0
                    };
                    initial2[m * k + i] = f64::from(mean as f32);
                }
            }
            let mut sample2 = Vec::with_capacity(sample_voxels.len() * m);
            for &v in &sample_voxels {
                for img in &self.intensities {
                    sample2.push(img[v]);
                }
            }
            let centers2 = self.kmeans(&sample2, m, &initial2)?;
            let labels2: Vec<u32> = sample2
                .par_chunks(m)
                .map(|x| {
                    let mut best = 0usize;
                    let mut min = 0.0f64;
                    for k in 0..classes {
                        let mut s = 0.0f64;
                        for i in 0..m {
                            let t = centers2[m * k + i] - f64::from(x[i]);
                            s += t * t;
                        }
                        let d = s.sqrt();
                        if k == 0 || d < min {
                            min = d;
                            best = k;
                        }
                    }
                    best as u32 + 1
                })
                .collect();
            if labels2 != labels1 {
                if fix {
                    for (i, &v) in sample_voxels.iter().enumerate() {
                        self.labels[v] = labels2[i];
                    }
                }
                self.notices.add(
                    &defects::ANTS_90,
                    "the initial labelling of several intensity images",
                );
            }
        }
        Ok(())
    }

    /// The k-means estimate of `samples` (`m` wide) from `initial`, as Atropos configures
    /// `KdTreeBasedKmeansEstimator` (bucket 16, 200 iterations, threshold 0), with ITK-50
    /// corrected unless reproduced.
    fn kmeans(&mut self, samples: &[f32], m: usize, initial: &[f64]) -> Result<Vec<f64>, AtroposError> {
        let tree = KdTree::new(samples, m, 16).map_err(|e| AtroposError::Refused(e.to_string()))?;
        let fix = self.options.upstream_defects.fix();
        let km = Kmeans {
            maximum_iteration: 200,
            threshold: 0.0,
            fix_split_points: fix,
        };
        let result = km.estimate(&tree, initial);
        if result.split_points_met {
            let other = Kmeans {
                fix_split_points: !fix,
                ..km
            }
            .estimate(&tree, initial);
            if other.parameters != result.parameters {
                self.notices.add(
                    &defects::ITK_50,
                    "the k-means centres of the initial labelling",
                );
            }
        }
        Ok(result.parameters)
    }

    /// `GenerateInitialClassLabelingWithOtsuThresholding`.
    fn otsu_labeling(&mut self) -> Result<(), AtroposError> {
        let classes = self.classes();
        let fix = self.options.upstream_defects.fix();
        let first = self.intensities[0];
        // ANTs: max from FLT_MIN, min from FLT_MAX, `<` else `>` (ANTS-91).
        let mut ants_max = f32::MIN_POSITIVE;
        let mut ants_min = f32::MAX;
        let mut true_min = f32::MAX;
        let mut true_max = -f32::MAX;
        for v in 0..self.n {
            if self.in_mask(v) {
                let x = first[v];
                if x < ants_min {
                    ants_min = x;
                } else if x > ants_max {
                    ants_max = x;
                }
                if x < true_min {
                    true_min = x;
                }
                if x > true_max {
                    true_max = x;
                }
            }
        }
        let (min_value, max_value) = if fix {
            (true_min, true_max)
        } else {
            (ants_min, ants_max)
        };
        if (ants_min, ants_max) != (true_min, true_max) {
            self.notices.add(
                &defects::ANTS_91,
                "the histogram range of the Otsu initialization",
            );
        }
        let thresholds_n = classes - 1;
        if thresholds_n >= 200 {
            return Err(AtroposError::Refused(format!(
                "Otsu with {classes} classes needs {thresholds_n} thresholds of a 200-bin \
                 histogram (ITK reads past the histogram)"
            )));
        }
        // LabelStatisticsImageFilter's histogram of label 1 (the mask's label 1, or ones).
        let mut h = larmorx_image::statistics::Histogram::new(
            200,
            f64::from(min_value),
            f64::from(max_value),
        );
        let mut any = false;
        for v in 0..self.n {
            if self.mask.map_or(1, |m| m[v]) == 1 {
                h.add(f64::from(first[v]), 1);
                any = true;
            }
        }
        if !any {
            return Err(AtroposError::Refused(
                "Otsu: the mask has no voxel labelled 1 (ANTs reads a histogram that does not \
                 exist)"
                    .into(),
            ));
        }
        let thresholds =
            larmorx_image::threshold::otsu_thresholds(&h, thresholds_n, false, false);
        let t: Vec<f32> = thresholds.iter().map(|&x| x as f32).collect();
        for v in 0..self.n {
            let mut label = 0u32;
            if self.in_mask(v) {
                let x = first[v];
                if x < t[0] {
                    label = 1;
                } else {
                    let mut found = false;
                    for i in 1..t.len() {
                        if x >= t[i - 1] && x <= t[i] {
                            label = i as u32 + 1;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        label = t.len() as u32 + 1;
                    }
                }
            }
            self.labels[v] = label;
        }
        Ok(())
    }

    /// `GenerateInitialClassLabelingWithPriorProbabilityImages`.
    fn prior_probability_labeling(&mut self) -> Result<(), AtroposError> {
        let classes = self.classes();
        let mut sum = vec![0.0f32; self.n];
        let mut max = vec![0.0f32; self.n];
        for c in 0..classes {
            let prior = self.prior(c).expect("prior").into_owned();
            for v in 0..self.n {
                sum[v] += prior[v];
            }
            for v in 0..self.n {
                if self.in_mask(v) {
                    let p = prior[v];
                    if p > max[v] {
                        max[v] = p;
                        self.labels[v] = c as u32 + 1;
                    } else if almost_equal_f32(p, max[v]) {
                        if c == 0 {
                            self.labels[v] = 1;
                        } else if integer_variate(&mut self.rng, 1) != 0 {
                            self.labels[v] = c as u32 + 1;
                        }
                    }
                }
            }
        }
        let threshold = self.options.probability_threshold;
        for c in 0..classes {
            // Each class's image as it is now (with aliased priors, the one image as the
            // previous classes' normalization left it).
            let mut prior = self.prior(c).expect("prior").into_owned();
            for v in 0..self.n {
                if self.in_mask(v) {
                    if max[v] <= threshold || almost_equal_f32(sum[v], 0.0) {
                        self.labels[v] = 0;
                        prior[v] = 0.0;
                    } else {
                        prior[v] /= sum[v];
                    }
                }
            }
            self.set_prior(c, prior);
        }
        self.prior_label = Some(self.labels.clone());
        Ok(())
    }

    // ---------------------------------------------------------------------------------------
    // Iterations

    /// `GetScalarSamples`: the masked voxels' intensities, each image's column through the
    /// outlier filter.
    ///
    /// The first image's column takes every value of the filter's output list: when the list
    /// keeps earlier calls' values (GrubbsRosner, ANTS-94), the sample grows by them and the
    /// later columns copy the list's first values.
    fn scalar_samples(&mut self) -> Result<(Vec<f32>, usize), AtroposError> {
        let m = self.intensities.len();
        let voxels: Vec<usize> = (0..self.n).filter(|&v| self.in_mask(v)).collect();
        let mut out: Vec<f32> = Vec::new();
        let mut size = voxels.len();
        let mut changed = false;
        for i in 0..m {
            let column: Vec<f32> = voxels.iter().map(|&v| self.intensities[i][v]).collect();
            let values: Vec<f32> = match self.outlier_filter.as_mut() {
                Some(f) => {
                    let (o, c) = f.apply(&column)?;
                    changed |= c;
                    o
                }
                None => column,
            };
            if i == 0 {
                size = values.len();
                out = vec![0.0f32; size * m];
            }
            for (j, &x) in values.iter().enumerate().take(size) {
                out[j * m + i] = x;
            }
        }
        if changed {
            self.notices.add(
                &defects::ANTS_94,
                "the outlier filter's output, which ANTs does not clear between calls",
            );
        }
        Ok((out, size))
    }

    /// `UpdateClassLabeling`: the ICM pass (asynchronous updating), the posteriors, the new
    /// labels and models; returns the posterior probability the iteration reports.
    fn update_class_labeling(&mut self) -> Result<f32, AtroposError> {
        let mut max_posterior_sum = 0.0f64;
        if self.options.asynchronous {
            if self.maximum_icm_code == 0 {
                self.compute_icm_codes();
            }
            let mut old = -1.0f64;
            let mut iterations = 0u32;
            let code_voxels = self.code_voxels();
            while max_posterior_sum > old && {
                let go = iterations < self.options.max_icm_iterations;
                iterations += 1;
                go
            } {
                old = max_posterior_sum;
                max_posterior_sum = 0.0;
                // Fisher-Yates over the codes 1..=max.
                let mut codes: Vec<u32> = (1..=self.maximum_icm_code).collect();
                for i in (1..codes.len()).rev() {
                    let j = integer_variate(&mut self.rng, i as u32) as usize;
                    codes.swap(i, j);
                }
                for &code in &codes {
                    let voxels = &code_voxels[code as usize - 1];
                    // The random labels in ANTs' order, then the updates (independent within
                    // one code).
                    let k = self.classes() as u32;
                    let random: Vec<u32> = voxels
                        .iter()
                        .map(|_| integer_variate(&mut self.rng, k - 1) + 1)
                        .collect();
                    let results: Vec<(u32, f32)> = {
                        let this = &*self;
                        larmorx_core::parallel::with_threads(this.n_threads, || {
                            voxels
                                .par_iter()
                                .zip(random.par_iter())
                                .with_min_len(256)
                                .map(|(&v, &r)| this.local_update(v, r))
                                .collect()
                        })
                        .map_err(|e| AtroposError::Refused(e.to_string()))?
                    };
                    for (&v, &(label, p)) in voxels.iter().zip(&results) {
                        self.labels[v] = label;
                        max_posterior_sum += f64::from(p);
                    }
                }
            }
        }
        let total = self.total_classes();
        let classes = self.classes();
        let mut max_posterior = vec![0.0f32; self.n];
        let mut max_labels = vec![0u32; self.n];
        let mut sum_posteriors = vec![0.0f64; total];
        let (sample, sample_size) = self.scalar_samples()?;
        let m = self.intensities.len();
        let voxels: Vec<usize> = (0..self.n).filter(|&v| self.in_mask(v)).collect();
        let reproduce_pv = self.options.upstream_defects.reproduce();
        let mut class_weights: Vec<Vec<f64>> = Vec::with_capacity(classes);
        for c in 0..total {
            let posterior = self.posterior(c)?;
            let mut weights = vec![0.0f64; sample_size];
            for (count, &v) in voxels.iter().enumerate() {
                let p = posterior[v];
                weights[count] = f64::from(p);
                if p > max_posterior[v] {
                    max_posterior[v] = p;
                    max_labels[v] = c as u32 + 1;
                } else if almost_equal_f32(p, max_posterior[v]) {
                    let current = max_labels[v];
                    if current >= 1 && self.label_volumes[c] < self.label_volumes[current as usize - 1]
                    {
                        max_labels[v] = c as u32 + 1;
                    }
                }
                sum_posteriors[c] += f64::from(p);
            }
            if c < classes {
                let w = self.components[c].set_sample(&sample, m, &weights)?;
                if let Some(w) = w {
                    self.warnings.push(w.to_string());
                }
                self.model_notices(c, &weights);
            } else {
                // ANTs' test `n == labelSet[d] - 1` never holds for a partial-volume class, so
                // the model is never re-estimated (ANTS-96); larmorx re-estimates it from the
                // posteriors of its two tissue classes.
                if reproduce_pv {
                    self.notices.add(
                        &defects::ANTS_96,
                        "the partial-volume classes keep their initial means",
                    );
                } else {
                    self.notices.add(
                        &defects::ANTS_96,
                        "the partial-volume classes re-estimated at every iteration",
                    );
                    let set = self.options.partial_volume_sets[c - classes].clone();
                    for (d, &l) in set.iter().enumerate() {
                        let pw = &class_weights[l as usize - 1];
                        self.components[c].set_indexed_sample(d, &sample, m, pw)?;
                    }
                }
                self.proportions[c] = 0.0;
            }
            if c < classes {
                class_weights.push(weights);
            }
            self.proportions[c] = if self.options.use_mixture_model_proportions {
                (sum_posteriors[c] / sample_size as f64) as f32
            } else {
                1.0 / total as f32
            };
        }
        // StatisticsImageFilter: the largest label must be the last class.
        let max_label = max_labels.iter().copied().max().unwrap_or(0);
        if max_label as usize != total {
            return Err(itk(
                1448,
                format!(
                    "Updating the class labeling resulted in {max_label} non-zero label sets but \
                     {total} is requested."
                ),
            ));
        }
        // LabelImageToShapeLabelMapFilter: the n-th label object's size.
        let mut counts: BTreeMap<u32, u64> = BTreeMap::new();
        for &l in &max_labels {
            if l != 0 {
                *counts.entry(l).or_insert(0) += 1;
            }
        }
        let objects: Vec<u64> = counts.values().copied().collect();
        for c in 0..total {
            match objects.get(c) {
                Some(&size) => self.label_volumes[c] = size,
                None => {
                    return Err(AtroposError::Itk {
                        file: "Modules/Filtering/LabelMap/include/itkLabelMap.hxx",
                        line: 180,
                        class: "LabelMap",
                        description: format!(
                            "Can't access label object at position {c}. The label map has only \
                             {} label objects registered.",
                            objects.len()
                        ),
                    });
                }
            }
        }
        self.labels = max_labels;
        if !self.options.asynchronous {
            max_posterior_sum = 0.0;
            for v in 0..self.n {
                if self.in_mask(v) {
                    max_posterior_sum += f64::from(max_posterior[v]);
                }
            }
        }
        Ok((max_posterior_sum / sample_size as f64) as f32)
    }

    /// The likelihood models' defects met by class `c`'s estimate from `weights`.
    fn model_notices(&mut self, c: usize, weights: &[f64]) {
        match &self.components[c] {
            Likelihood::ManifoldParzen(model) => {
                if weights.windows(2).any(|w| w[0] != w[1]) {
                    self.notices.add(
                        &defects::ANTS_95,
                        "the manifold Parzen likelihoods, evaluated with the posterior weights",
                    );
                }
                if model.met_covariance_defect {
                    self.notices.add(
                        &defects::ANTS_104,
                        "the manifold Parzen windows' local covariances",
                    );
                }
            }
            Likelihood::JointShape(_) => {
                self.notices.add(
                    &defects::ANTS_103,
                    "the joint shape and orientation likelihood reads an unset coordinate (0 here)",
                );
            }
            _ => {}
        }
    }

    /// The masked voxels of each ICM code, in raster order.
    fn code_voxels(&self) -> Vec<Vec<usize>> {
        let codes = self.icm_codes.as_ref().expect("ICM codes");
        let mut out = vec![Vec::new(); self.maximum_icm_code as usize];
        for (v, &c) in codes.iter().enumerate() {
            if c >= 1 && c <= self.maximum_icm_code {
                out[c as usize - 1].push(v);
            }
        }
        out
    }

    /// `ComputeICMCodeImage`: codes `1, 2, …` assigned in raster order so that no
    /// neighbourhood holds a code twice.
    fn compute_icm_codes(&mut self) {
        let mut codes = vec![0u32; self.n];
        let mut max_code = 1u32;
        let mut coord = vec![0usize; self.size.len()];
        let nb = self.neighborhood.clone();
        loop {
            let mut complete = true;
            for v in 0..self.n {
                if codes[v] == 0 && self.in_mask(v) {
                    self.coord(v, &mut coord);
                    let mut has = false;
                    'elements: for k in 0..nb.len() {
                        for (a, &o) in nb.offsets[k].iter().enumerate() {
                            let c = coord[a] as i64 + o;
                            if c < 0 || c >= self.size[a] as i64 {
                                continue 'elements;
                            }
                        }
                        if codes[(v as i64 + nb.linear[k]) as usize] == max_code {
                            has = true;
                            break;
                        }
                    }
                    if !has {
                        codes[v] = max_code;
                        complete = false;
                    }
                }
            }
            if complete {
                break;
            }
            max_code += 1;
        }
        self.maximum_icm_code = max_code - 1;
        self.icm_codes = Some(codes);
    }

    /// The ICM code image (`GetICMCodeImage`), once computed.
    pub fn icm_codes(&self) -> Option<&[u32]> {
        self.icm_codes.as_deref()
    }

    /// The measurement vector at `v`.
    #[inline]
    fn measurement(&self, v: usize, out: &mut [f32]) {
        for (i, img) in self.intensities.iter().enumerate() {
            out[i] = img[v];
        }
    }

    /// `PerformLocalLabelingUpdate` at `v`, with the label ANTs drew for it: the new label and
    /// the normalized maximum posterior.
    fn local_update(&self, v: usize, random_label: u32) -> (u32, f32) {
        let total = self.total_classes();
        let classes = self.classes();
        let m = self.intensities.len();
        let mut x = vec![0.0f32; m];
        self.measurement(v, &mut x);
        let s = match &self.options.mrf_coefficients {
            Some(c) => c[v],
            None => self.options.mrf_smoothing,
        };
        let mut coord = vec![0usize; self.size.len()];
        self.coord(v, &mut coord);
        let mut w = vec![0.0f32; total];
        let mut h = vec![0.0f32; total];
        self.mrf_weights(&self.labels, v, &coord, &mut w, &mut h);
        let mut max_label = random_label;
        let mut max_p = 0.0f32;
        let mut sum_p = 0.0f32;
        let use_mrf = s > 0.0 && self.neighborhood.len() > 1;
        let exps: Vec<f32> = if use_mrf {
            w.iter().map(|&wk| expf(-s * wk)).collect()
        } else {
            Vec::new()
        };
        for k in 0..total {
            let likelihood = self.components[k].evaluate(&x);
            let mut mrf = 1.0f32;
            if use_mrf {
                let numerator = exps[k];
                let mut denominator = 0.0f32;
                for e in exps.iter().take(classes) {
                    denominator += e;
                }
                if denominator > 0.0 {
                    mrf = numerator / denominator;
                }
            }
            let prior = match self.prior_value(k, v) {
                Some(p) => p,
                None => 1.0,
            };
            let mut p = self.local_posterior(
                self.proportions[k],
                prior,
                1.0,
                mrf,
                likelihood,
                v,
                k as u32 + 1,
            );
            if !p.is_finite() {
                p = 0.0;
            }
            sum_p += p;
            if p > max_p {
                max_p = p;
                max_label = k as u32 + 1;
            }
        }
        let mut r = max_p / sum_p;
        if !r.is_finite() {
            r = 0.0;
        }
        (max_label, r)
    }

    /// One voxel's value of `GetPriorProbabilityImage(k + 1)`, if that image exists.
    #[inline]
    fn prior_value(&self, k: usize, v: usize) -> Option<f32> {
        if !self.uses_prior_probabilities() || k >= self.classes() {
            return None;
        }
        if self.options.minimize_memory {
            // The sparse list, as reconstructed: the last entry at `v` wins.
            let list = &self.sparse_priors[self.slot(k)];
            let mut value = 0.0f32;
            if let Ok(i) = list.binary_search_by(|&(n, _)| n.cmp(&(v as u64))) {
                // Entries may repeat a rounded number (ANTS-97): take the last one.
                let mut j = i;
                while j + 1 < list.len() && list[j + 1].0 == v as u64 {
                    j += 1;
                }
                value = list[j].1;
            }
            Some(value)
        } else {
            Some(self.priors[self.slot(k)][v])
        }
    }

    /// `CalculateLocalPosteriorProbability`.
    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn local_posterior(
        &self,
        _proportion: f32,
        spatial: f32,
        distance: f32,
        mrf: f32,
        likelihood: f32,
        v: usize,
        class: u32,
    ) -> f32 {
        let o = self.options;
        let eps = 1.0e-10f32;
        let mut spatial = max_f(eps, spatial);
        let distance = max_f(eps, distance);
        let mut mrf = max_f(eps, mrf);
        let mut likelihood = max_f(eps, likelihood);
        let w = o.prior_weight;
        let mut p = match o.formulation {
            Formulation::Socrates => powf(spatial, w) * powf(likelihood * mrf, 1.0 - w),
            Formulation::Plato => {
                let label_hit = self.uses_prior_label()
                    && self.prior_label.as_ref().is_some_and(|l| l[v] == class);
                let prior_hit = !label_hit
                    && self
                        .prior_value(class as usize - 1, v)
                        .is_some_and(|p| almost_equal_f32(p, 1.0));
                if label_hit || prior_hit {
                    spatial = 1.0;
                    likelihood = 1.0;
                    mrf = 1.0;
                }
                powf(spatial, w) * powf(likelihood * mrf, 1.0 - w)
            }
            Formulation::Aristotle => {
                powf(spatial * distance, w) * powf(likelihood * mrf, 1.0 - w)
            }
            Formulation::Sigmoid => {
                let total = self.total_classes() as f32;
                let offset = 1.0 / total;
                let s = 1.0 / (1.0 + expf(-1.0 * (spatial - offset) * w));
                s * (likelihood * mrf)
            }
        };
        if !almost_equal_f32(o.initial_annealing_temperature, 1.0) {
            let t = o.initial_annealing_temperature
                * powf(o.annealing_rate, self.elapsed_iterations as f32);
            let t = max_f(t, o.minimum_annealing_temperature);
            p = powf(p, 1.0 / t);
        }
        if !p.is_finite() {
            p = 0.0;
        }
        p
    }

    // ---------------------------------------------------------------------------------------
    // Posteriors

    /// `GetPosteriorProbabilityImage(c + 1)` as `UpdateClassLabeling` and `Atropos.cxx` call
    /// it, in class order: class 1 computes and normalizes every class (stored unless memory
    /// is minimized); later classes are read back, or computed alone.
    pub fn posterior(&mut self, c: usize) -> Result<Vec<f32>, AtroposError> {
        let total = self.total_classes();
        if c >= total {
            return Err(itk(1996, "Requested class is greater than the number of classes."));
        }
        if c < self.posteriors.len() {
            return Ok(self.posteriors[c].clone());
        }
        if c == 0 {
            let sum_prior = if self.uses_priors() {
                Some(self.sum_prior_image(total))
            } else {
                None
            };
            let mut sum = vec![0.0f32; self.n];
            let mut first = Vec::new();
            for k in 0..total {
                let smooth = self.smooth_images_for(k)?;
                let distance = self.distance_prior(k)?;
                let p = self.posterior_values(
                    k,
                    sum_prior.as_deref(),
                    distance.as_deref(),
                    &smooth,
                    true,
                )?;
                for v in 0..self.n {
                    if self.in_mask(v) {
                        sum[v] += p[v];
                    }
                }
                if !self.options.minimize_memory {
                    self.posteriors.push(p);
                } else if k == 0 {
                    first = p;
                }
            }
            self.sum_posterior = sum;
            if self.options.minimize_memory {
                for v in 0..self.n {
                    if self.sum_posterior[v] > 0.0 {
                        first[v] /= self.sum_posterior[v];
                    }
                }
                Ok(first)
            } else {
                for k in 0..total {
                    for v in 0..self.n {
                        if self.sum_posterior[v] > 0.0 {
                            self.posteriors[k][v] /= self.sum_posterior[v];
                        }
                    }
                }
                Ok(self.posteriors[0].clone())
            }
        } else {
            let sum_prior = if self.uses_priors() {
                Some(self.sum_prior_image(total))
            } else {
                None
            };
            let smooth = if self.uses_priors() {
                self.smooth_images_for_any(c)?
            } else {
                Vec::new()
            };
            let distance = self.distance_prior(c)?;
            let mut p = self.posterior_values(
                c,
                sum_prior.as_deref(),
                distance.as_deref(),
                &smooth,
                false,
            )?;
            for v in 0..self.n {
                if self.sum_posterior[v] > 0.0 {
                    p[v] /= self.sum_posterior[v];
                }
            }
            Ok(p)
        }
    }

    /// The sum of the priors weighted by the mixing proportions (`sumPriorProbabilityImage`).
    fn sum_prior_image(&self, total: usize) -> Vec<f32> {
        let mut sum = vec![0.0f32; self.n];
        for c in 0..total {
            let Some(prior) = self.prior(c) else {
                continue;
            };
            for v in 0..self.n {
                sum[v] += self.proportions[c] * prior[v];
            }
        }
        sum
    }

    /// The smoothed intensity images of class `c` for the class-1 pass (prior probability
    /// images, or a prior label image for the tissue classes).
    fn smooth_images_for(&mut self, c: usize) -> Result<Vec<Option<Vec<f32>>>, AtroposError> {
        let m = self.intensities.len();
        if self.uses_prior_probabilities() || (self.uses_prior_label() && c < self.classes()) {
            let mut out = Vec::with_capacity(m);
            for i in 0..m {
                if self.options.adaptive_smoothing.get(i).is_some_and(|&w| w > 0.0) {
                    out.push(Some(self.smooth_intensity(i, c)?));
                } else {
                    out.push(None);
                }
            }
            Ok(out)
        } else {
            Ok(Vec::new())
        }
    }

    /// The smoothed images of the later classes' pass (any prior-based initialization).
    fn smooth_images_for_any(&mut self, c: usize) -> Result<Vec<Option<Vec<f32>>>, AtroposError> {
        let m = self.intensities.len();
        let mut out = Vec::with_capacity(m);
        for i in 0..m {
            if self.options.adaptive_smoothing.get(i).is_some_and(|&w| w > 0.0) {
                out.push(Some(self.smooth_intensity(i, c)?));
            } else {
                out.push(None);
            }
        }
        Ok(out)
    }

    /// The unnormalized posterior of class `c` at every masked voxel (0 elsewhere).
    fn posterior_values(
        &self,
        c: usize,
        sum_prior: Option<&[f32]>,
        distance: Option<&[f32]>,
        smooth: &[Option<Vec<f32>>],
        first_pass: bool,
    ) -> Result<Vec<f32>, AtroposError> {
        let total = self.total_classes();
        let m = self.intensities.len();
        let prior_img = self.prior(c);
        let prior_img = prior_img.as_deref();
        let uses_priors = self.uses_priors();
        let threshold = self.options.probability_threshold;
        let weights = &self.options.adaptive_smoothing;
        let mut out = vec![0.0f32; self.n];
        larmorx_core::parallel::with_threads(self.n_threads, || {
            out.par_chunks_mut(4096)
                .enumerate()
                .for_each(|(chunk, slice)| {
                    let mut coord = vec![0usize; self.size.len()];
                    let mut w = vec![0.0f32; total];
                    let mut h = vec![0.0f32; total];
                    let mut x = vec![0.0f32; m];
                    for (j, slot) in slice.iter_mut().enumerate() {
                        let v = chunk * 4096 + j;
                        if !self.in_mask(v) {
                            continue;
                        }
                        let s = match &self.options.mrf_coefficients {
                            Some(cf) => cf[v],
                            None => self.options.mrf_smoothing,
                        };
                        let mut mrf = 1.0f32;
                        if s > 0.0 && self.neighborhood.len() > 1 {
                            self.coord(v, &mut coord);
                            self.mrf_weights(&self.labels, v, &coord, &mut w, &mut h);
                            let numerator = expf(-s * w[c]);
                            let mut denominator = 0.0f32;
                            for &wk in w.iter() {
                                denominator += expf(-s * wk);
                            }
                            if denominator > 0.0 {
                                mrf = numerator / denominator;
                            }
                        }
                        let mut prior = 0.0f32;
                        let mut distance_prior = 0.0f32;
                        if uses_priors {
                            if let Some(d) = distance {
                                distance_prior = d[v];
                            }
                            if let Some(p) = prior_img {
                                prior = p[v];
                            } else if first_pass && self.prior_label.is_some() {
                                // Only the class-1 pass has this branch (the later classes'
                                // pass, with --minimize-memory-usage, keeps 0).
                                prior = 1.0 / total as f32;
                            }
                            let sp = sum_prior.map_or(0.0, |s| s[v]);
                            if sp > threshold {
                                prior *= self.proportions[c] / sp;
                            } else if distance.is_some() {
                                prior = distance_prior;
                            }
                        }
                        self.measurement(v, &mut x);
                        if uses_priors {
                            for (i, sm) in smooth.iter().enumerate() {
                                if let Some(sm) = sm {
                                    x[i] = (1.0 - weights[i]) * x[i] + weights[i] * sm[v];
                                }
                            }
                        }
                        let likelihood = self.components[c].evaluate(&x);
                        let p = self.local_posterior(
                            self.proportions[c],
                            prior,
                            distance_prior,
                            mrf,
                            likelihood,
                            v,
                            c as u32 + 1,
                        );
                        *slot = if p.is_finite() { p } else { 0.0 };
                    }
                });
        })
        .map_err(|e| AtroposError::Refused(e.to_string()))?;
        Ok(out)
    }

    // ---------------------------------------------------------------------------------------
    // Distance priors

    /// `GetDistancePriorProbabilityImage(c + 1)`: `None` without prior-based initialization or
    /// for a partial-volume class.
    pub fn distance_prior(&mut self, c: usize) -> Result<Option<Vec<f32>>, AtroposError> {
        let classes = self.classes();
        if !self.uses_priors() || c >= classes {
            return Ok(None);
        }
        if c < self.distance_priors.len() {
            return Ok(Some(self.distance_priors[c].clone()));
        }
        let threshold = self.options.probability_threshold;
        if c == 0 {
            let mut sum = vec![0.0f32; self.n];
            let mut first = Vec::new();
            for k in 0..classes {
                let d = self.label_distance_probability(k)?;
                for v in 0..self.n {
                    sum[v] += d[v];
                }
                if !self.options.minimize_memory {
                    self.distance_priors.push(d);
                } else if k == 0 {
                    first = d;
                }
            }
            self.sum_distance_prior = sum;
            let normalize = |img: &mut Vec<f32>, this: &Self| {
                for v in 0..this.n {
                    if this.in_mask(v) {
                        let s = this.sum_distance_prior[v];
                        img[v] = if s <= threshold { 0.0 } else { img[v] / s };
                    }
                }
            };
            if self.options.minimize_memory {
                normalize(&mut first, self);
                Ok(Some(first))
            } else {
                let mut all = std::mem::take(&mut self.distance_priors);
                for img in all.iter_mut() {
                    normalize(img, self);
                }
                self.distance_priors = all;
                Ok(Some(self.distance_priors[0].clone()))
            }
        } else {
            let mut d = self.label_distance_probability(c)?;
            for v in 0..self.n {
                if self.in_mask(v) {
                    let s = self.sum_distance_prior[v];
                    d[v] = if s <= threshold { 0.0 } else { d[v] / s };
                }
            }
            Ok(Some(d))
        }
    }

    /// The unnormalized distance prior of class `c`: the signed distance to the label (Maurer,
    /// or fast marching along the mask), turned into a probability with the class's lambda and
    /// boundary probability.
    fn label_distance_probability(&self, c: usize) -> Result<Vec<f32>, AtroposError> {
        let source: &[u32] = if self.uses_prior_label() {
            self.prior_label.as_deref().expect("prior label")
        } else {
            &self.labels
        };
        let label = c as u32 + 1;
        let binary: Vec<f32> = source.iter().map(|&l| if l == label { 1.0 } else { 0.0 }).collect();
        let mut distance = if self.options.euclidean_distance {
            let options = larmorx_image::distance::MaurerOptions {
                background: 0.0,
                inside_is_positive: false,
                squared: false,
                use_spacing: true,
            };
            larmorx_image::distance::signed_maurer_distance_map(
                larmorx_image::VolumeRef::new(&binary, &self.size, &self.spacing),
                &options,
                self.n_threads,
            )
            .map_err(|e| AtroposError::Refused(e.to_string()))?
        } else {
            let as_labels: Vec<u64> = binary.iter().map(|&b| b as u64).collect();
            let contour = larmorx_image::components::label_contour(
                &as_labels,
                &self.size,
                false,
                0,
                self.n_threads,
            )
            .map_err(|e| AtroposError::Refused(e.to_string()))?;
            let trial: Vec<usize> = contour
                .iter()
                .enumerate()
                .filter(|(_, on)| **on)
                .map(|(i, _)| i)
                .collect();
            let speed: Option<Vec<f32>> = self.mask.map(|m| m.iter().map(|&x| x as f32).collect());
            let mut out = fast_marching(&self.size, &self.spacing, speed.as_deref(), &trial)
                .map_err(|e| AtroposError::Itk {
                    file: "Modules/Filtering/FastMarching/include/itkFastMarchingImageFilter.hxx",
                    line: 0,
                    class: "FastMarchingImageFilter",
                    description: e.0.to_string(),
                })?;
            for (o, &b) in out.iter_mut().zip(&binary) {
                if almost_equal_f32(b, 1.0) {
                    *o = -*o;
                }
            }
            out
        };
        let mut maximum_interior = 0.0f32;
        for &d in &distance {
            if d < 0.0 && maximum_interior < d.abs() {
                maximum_interior = d.abs();
            }
        }
        let (lambda, boundary) = self
            .options
            .label_parameters
            .get(&label)
            .copied()
            .unwrap_or((0.0, 1.0));
        for d in distance.iter_mut() {
            if almost_equal_f32(lambda, 0.0) {
                *d = if *d <= 0.0 { boundary } else { 0.0 };
            } else if *d >= 0.0 {
                *d = boundary * expf(-lambda * *d);
            } else if *d < 0.0 {
                *d = 1.0 - (1.0 - boundary) * (maximum_interior - d.abs()) / maximum_interior;
            }
        }
        Ok(distance)
    }

    // ---------------------------------------------------------------------------------------
    // Adaptive smoothing and likelihood images

    /// `GetSmoothIntensityImageFromPriorImage(i, c + 1)`.
    pub fn smooth_intensity(&mut self, i: usize, c: usize) -> Result<Vec<f32>, AtroposError> {
        let order = self.options.spline_order as usize;
        let smoothing_error = |e: super::smoothing::SmoothingError| AtroposError::Itk {
            file: "Modules/Filtering/ImageGrid/include/itkBSplineScatteredDataPointSetToImageFilter.hxx",
            line: 0,
            class: "BSplineScatteredDataPointSetToImageFilter",
            description: e.0,
        };
        if c >= self.classes() || i >= self.lattices.len() {
            return Err(AtroposError::Refused(format!(
                "no control-point lattice for image {i} and class {} (ANTs reads past its list)",
                c + 1
            )));
        }
        if let Some(lattice) = &self.lattices[i][c] {
            return super::smoothing::reconstruct(
                lattice,
                order,
                &self.size,
                &self.spacing,
                self.n_threads,
            )
            .map_err(smoothing_error);
        }
        let probability: Vec<f32> = if self.uses_prior_probabilities() {
            self.prior(c).expect("prior").into_owned()
        } else {
            let label = c as u32 + 1;
            self.prior_label
                .as_ref()
                .expect("prior label")
                .iter()
                .map(|&l| if l == label { 1.0 } else { 0.0 })
                .collect()
        };
        let mut voxels = Vec::new();
        let mut values = Vec::new();
        let mut weights = Vec::new();
        for v in 0..self.n {
            if self.in_mask(v) && probability[v] >= 0.5 {
                voxels.push(v);
                values.push(self.intensities[i][v]);
                weights.push(probability[v]);
            }
        }
        let settings = super::smoothing::FitSettings {
            origin: &self.origin,
            spacing: &self.spacing,
            size: &self.size,
            order,
            levels: &self.options.levels,
            control_points: &self.options.control_points,
        };
        let lattice = super::smoothing::fit(&settings, &voxels, &values, &weights, self.n_threads)
            .map_err(smoothing_error)?;
        let out = super::smoothing::reconstruct(
            &lattice,
            order,
            &self.size,
            &self.spacing,
            self.n_threads,
        )
        .map_err(smoothing_error)?;
        self.lattices[i][c] = Some(lattice);
        Ok(out)
    }

    /// `GetLikelihoodImage(c + 1)`.
    pub fn likelihood(&mut self, c: usize) -> Result<Vec<f32>, AtroposError> {
        let m = self.intensities.len();
        let smooth = if self.uses_priors() {
            self.smooth_images_for_any(c)?
        } else {
            Vec::new()
        };
        let weights = &self.options.adaptive_smoothing;
        let mut out = vec![0.0f32; self.n];
        let this = &*self;
        larmorx_core::parallel::with_threads(this.n_threads, || {
            out.par_chunks_mut(4096).enumerate().for_each(|(chunk, slice)| {
                let mut x = vec![0.0f32; m];
                for (j, slot) in slice.iter_mut().enumerate() {
                    let v = chunk * 4096 + j;
                    if !this.in_mask(v) {
                        continue;
                    }
                    this.measurement(v, &mut x);
                    for (i, sm) in smooth.iter().enumerate() {
                        if let Some(sm) = sm {
                            x[i] = (1.0 - weights[i]) * x[i] + weights[i] * sm[v];
                        }
                    }
                    *slot = this.components[c].evaluate(&x);
                }
            });
        })
        .map_err(|e| AtroposError::Refused(e.to_string()))?;
        Ok(out)
    }

    /// The tissue classes' models, for printing.
    pub fn components(&self) -> &[Likelihood] {
        &self.components
    }

    /// Whether a prior probability image exists for class `c` (`GetPriorProbabilityImage`).
    pub fn has_prior(&self, c: usize) -> bool {
        self.uses_prior_probabilities() && c < self.classes()
    }

    /// Whether a prior label image exists (`GetPriorLabelImage`): the given one, or the
    /// initial labelling made from prior probability images.
    pub fn has_prior_label(&self) -> bool {
        self.prior_label.is_some()
    }
}
