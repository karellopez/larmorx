// SPDX-License-Identifier: Apache-2.0
//! ITK's k-d tree, its nearest-neighbour search and its k-means estimator (ITK v5.4.5,
//! `Modules/Numerics/Statistics`): `WeightedCentroidKdTreeGenerator` (`itkKdTreeGenerator.hxx`,
//! `itkWeightedCentroidKdTreeGenerator.hxx`, `itkStatisticsAlgorithm.hxx`), `KdTree::Search`
//! (`itkKdTree.hxx`) and `KdTreeBasedKmeansEstimator` (`itkKdTreeBasedKmeansEstimator.hxx`), as
//! Atropos uses them on a `ListSample<Array<float>>`.
//!
//! **The tree.** The sample is held as instance identifiers (`Subsample`); a node with more
//! than `bucket` instances is split at the median of its most widely spread axis (the last axis
//! of the largest spread, `>=`), found with ITK's `NthElement` (median-of-three partitions,
//! then insertion sort), which reorders the identifiers in place: the reordering decides the
//! leaves' contents and order, and with them the order of every sum, so it is reproduced
//! operation for operation. The split instance stays in the inner node; its weighted centroid
//! (the sum of its instances in `f64`, added in the subsample's order before the split) and
//! centroid (that sum divided by the count) are kept for k-means.
//!
//! **k-means** (Kanungo et al.'s filtering algorithm): each pass sends every node to the
//! candidate centres that can be closest to some point of its cell, pruning the others
//! (`IsFarther`), and adds a whole node to its centre when one candidate remains. **ITK drops
//! the split instance of every inner node it recurses into** (defect ITK-50: those instances
//! are counted nowhere in that pass); [`Kmeans::fix_split_points`] counts them (larmorx's
//! default in Atropos), `false` reproduces ITK.
//!
//! Measurements are `f32`; distances, centres and sums are `f64`, as in ITK. Everything is
//! sequential and in ITK's order, so the results do not depend on threads or platforms.

use crate::FilterError;

/// A node of the tree.
#[derive(Clone, Debug)]
enum Node {
    /// The shared empty leaf (`GetEmptyTerminalNode`).
    Empty,
    /// A leaf: instance identifiers in the subsample's order.
    Leaf(Vec<u32>),
    /// An inner node (`KdTreeWeightedCentroidNonterminalNode`).
    Inner {
        dim: usize,
        value: f32,
        /// The split instance (`m_InstanceIdentifier`).
        id: u32,
        left: usize,
        right: usize,
        /// Sum of the node's instances, the split one included.
        weighted_centroid: Vec<f64>,
        centroid: Vec<f64>,
        size: u32,
    },
}

/// A k-d tree over `n` measurement vectors of length `m` (row-major `samples`, copied).
#[derive(Clone, Debug)]
pub struct KdTree {
    samples: Vec<f32>,
    m: usize,
    nodes: Vec<Node>,
    root: usize,
}

impl KdTree {
    /// `WeightedCentroidKdTreeGenerator` with bucket size `bucket` (Atropos uses 16) on the
    /// sample `samples` (`n × m`, row-major).
    pub fn new(samples: &[f32], m: usize, bucket: usize) -> Result<Self, FilterError> {
        if m == 0 || !samples.len().is_multiple_of(m) {
            return Err(FilterError::invalid(
                "Length of a sample's measurement vector hasn't been set.",
            ));
        }
        let n = samples.len() / m;
        if n > u32::MAX as usize {
            return Err(FilterError::invalid("too many instances for a k-d tree"));
        }
        let mut ids: Vec<u32> = (0..n as u32).collect();
        let mut tree = KdTree {
            samples: samples.to_vec(),
            m,
            nodes: vec![Node::Empty],
            root: 0,
        };
        let mut lower = vec![-f32::MAX; m];
        let mut upper = vec![f32::MAX; m];
        let root = tree.generate(&mut ids, 0, n, &mut lower, &mut upper, bucket.max(1));
        tree.root = root;
        Ok(tree)
    }

    /// The number of instances.
    pub fn len(&self) -> usize {
        self.samples.len() / self.m
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// The measurement vector of instance `id`.
    #[inline]
    pub fn vector(&self, id: u32) -> &[f32] {
        let s = id as usize * self.m;
        &self.samples[s..s + self.m]
    }

    /// `GenerateTreeLoop`.
    fn generate(
        &mut self,
        ids: &mut [u32],
        begin: usize,
        end: usize,
        lower: &mut [f32],
        upper: &mut [f32],
        bucket: usize,
    ) -> usize {
        if end - begin <= bucket {
            if end == begin {
                return 0;
            }
            self.nodes.push(Node::Leaf(ids[begin..end].to_vec()));
            return self.nodes.len() - 1;
        }
        self.generate_inner(ids, begin, end, lower, upper, bucket)
    }

    /// `WeightedCentroidKdTreeGenerator::GenerateNonterminalNode`.
    fn generate_inner(
        &mut self,
        ids: &mut [u32],
        begin: usize,
        end: usize,
        lower: &mut [f32],
        upper: &mut [f32],
        bucket: usize,
    ) -> usize {
        let m = self.m;
        let mut weighted = vec![0.0f64; m];
        for &id in &ids[begin..end] {
            let v = self.vector(id);
            for (w, &x) in weighted.iter_mut().zip(v) {
                *w += f64::from(x);
            }
        }
        // FindSampleBoundAndMean (the mean is not used).
        let first = self.vector(ids[begin]).to_vec();
        let mut min = first.clone();
        let mut max = first;
        for &id in &ids[begin + 1..end] {
            let v = self.vector(id);
            for d in 0..m {
                if v[d] < min[d] {
                    min[d] = v[d];
                } else if v[d] > max[d] {
                    max[d] = v[d];
                }
            }
        }
        let mut max_spread = -f32::MAX;
        let mut dim = 0;
        for d in 0..m {
            let spread = max[d] - min[d];
            if spread >= max_spread {
                max_spread = spread;
                dim = d;
            }
        }
        let median = (end - begin) / 2;
        let value = nth_element(&self.samples, m, ids, dim, begin, end, median);
        let median = median + begin;
        let saved_lower = lower[dim];
        let saved_upper = upper[dim];
        upper[dim] = value;
        let left = self.generate(ids, begin, median, lower, upper, bucket);
        upper[dim] = saved_upper;
        lower[dim] = value;
        let right = self.generate(ids, median + 1, end, lower, upper, bucket);
        lower[dim] = saved_lower;
        let size = (end - begin) as u32;
        let centroid = weighted.iter().map(|&w| w / f64::from(size)).collect();
        self.nodes.push(Node::Inner {
            dim,
            value,
            id: ids[median],
            left,
            right,
            weighted_centroid: weighted,
            centroid,
            size,
        });
        self.nodes.len() - 1
    }

    /// `KdTree::Search(query, k, result)`: the identifiers of the `k` nearest instances, in
    /// the order ITK's `NearestNeighbors` holds them (slot order, each new neighbour replacing
    /// the farthest so far). Errors as ITK when `k` exceeds the number of instances.
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<u32>, FilterError> {
        let mut ids = Vec::new();
        let mut distances = Vec::new();
        self.search_into(query, k, &mut ids, &mut distances)?;
        Ok(ids)
    }

    /// [`KdTree::search`] into reusable buffers (`ids` gets the result).
    pub fn search_into(
        &self,
        query: &[f32],
        k: usize,
        ids: &mut Vec<u32>,
        distances: &mut Vec<f64>,
    ) -> Result<(), FilterError> {
        if k > self.len() {
            return Err(FilterError::invalid(
                "The numberOfNeighborsRequested for the nearest neighbor search should be less \
                 than or equal to the number of the measurement vectors.",
            ));
        }
        ids.clear();
        ids.resize(k, u32::MAX);
        distances.clear();
        distances.resize(k, f64::MAX);
        let mut nn = Neighbors {
            farthest: 0,
            ids,
            distances,
        };
        // lowerBound = float(-sqrt(-double(-FLT_MAX)) / 2), upperBound = float(sqrt(FLT_MAX / 2)).
        let lo = (-(f64::from(f32::MAX)).sqrt() / 2.0) as f32;
        let hi = (f64::from(f32::MAX) / 2.0).sqrt() as f32;
        let mut lower = vec![lo; self.m];
        let mut upper = vec![hi; self.m];
        self.nn_loop(self.root, query, &mut lower, &mut upper, &mut nn);
        Ok(())
    }

    /// `EuclideanDistanceMetric<Array<float>>::Evaluate(x1, x2)`: differences in `f32`,
    /// squares and sum in `f64`.
    #[inline]
    fn distance(a: &[f32], b: &[f32]) -> f64 {
        let mut s = 0.0f64;
        for (x, y) in a.iter().zip(b) {
            let t = f64::from(x - y);
            s += t * t;
        }
        s.sqrt()
    }

    /// `NearestNeighborSearchLoop`.
    fn nn_loop(
        &self,
        node: usize,
        query: &[f32],
        lower: &mut [f32],
        upper: &mut [f32],
        nn: &mut Neighbors<'_>,
    ) -> bool {
        match &self.nodes[node] {
            Node::Empty => false,
            Node::Leaf(ids) => {
                for &id in ids {
                    let d = Self::distance(query, self.vector(id));
                    if d < nn.largest() {
                        nn.replace_farthest(id, d);
                    }
                }
                self.ball_within_bounds(query, lower, upper, nn.largest())
            }
            Node::Inner {
                dim,
                value,
                id,
                left,
                right,
                ..
            } => {
                let (dim, value) = (*dim, *value);
                let d = Self::distance(query, self.vector(*id));
                if d < nn.largest() {
                    nn.replace_farthest(*id, d);
                }
                if query[dim] <= value {
                    let t = upper[dim];
                    upper[dim] = value;
                    if self.nn_loop(*left, query, lower, upper, nn) {
                        return true;
                    }
                    upper[dim] = t;
                    let t = lower[dim];
                    lower[dim] = value;
                    if self.bounds_overlap_ball(query, lower, upper, nn.largest()) {
                        self.nn_loop(*right, query, lower, upper, nn);
                    }
                    lower[dim] = t;
                } else {
                    let t = lower[dim];
                    lower[dim] = value;
                    if self.nn_loop(*right, query, lower, upper, nn) {
                        return true;
                    }
                    lower[dim] = t;
                    let t = upper[dim];
                    upper[dim] = value;
                    if self.bounds_overlap_ball(query, lower, upper, nn.largest()) {
                        self.nn_loop(*left, query, lower, upper, nn);
                    }
                    upper[dim] = t;
                }
                self.ball_within_bounds(query, lower, upper, nn.largest())
            }
        }
    }

    /// `BallWithinBounds`: per axis, `|q − bound|` in `f32` then `f64`.
    fn ball_within_bounds(&self, query: &[f32], lower: &[f32], upper: &[f32], radius: f64) -> bool {
        for d in 0..self.m {
            if f64::from(query[d] - lower[d]).abs() <= radius
                || f64::from(query[d] - upper[d]).abs() <= radius
            {
                return false;
            }
        }
        true
    }

    /// `BoundsOverlapBall`, with ITK's early return after each axis outside the box.
    fn bounds_overlap_ball(
        &self,
        query: &[f32],
        lower: &[f32],
        upper: &[f32],
        radius: f64,
    ) -> bool {
        let r2 = radius * radius;
        let mut sum = 0.0f64;
        for d in 0..self.m {
            if query[d] <= lower[d] {
                let t = f64::from(query[d] - lower[d]).abs();
                sum += t * t;
                if sum < r2 {
                    return true;
                }
            } else if query[d] >= upper[d] {
                let t = f64::from(query[d] - upper[d]).abs();
                sum += t * t;
                if sum < r2 {
                    return true;
                }
            }
        }
        false
    }
}

/// `KdTree::NearestNeighbors`.
struct Neighbors<'b> {
    farthest: usize,
    ids: &'b mut Vec<u32>,
    distances: &'b mut Vec<f64>,
}

impl Neighbors<'_> {
    #[inline]
    fn largest(&self) -> f64 {
        self.distances[self.farthest]
    }

    /// `ReplaceFarthestNeighbor`: the new farthest is the first largest distance above
    /// `DBL_MIN` (ITK starts from `NumericTraits<double>::min()`, the smallest positive
    /// normal; when every distance is 0 the index stays).
    fn replace_farthest(&mut self, id: u32, distance: f64) {
        self.ids[self.farthest] = id;
        self.distances[self.farthest] = distance;
        let mut far = f64::MIN_POSITIVE;
        for (i, &d) in self.distances.iter().enumerate() {
            if d > far {
                far = d;
                self.farthest = i;
            }
        }
    }
}

/// `Algorithm::NthElement` on the subsample `ids` (`samples` are `m` wide): reorders
/// `ids[begin..end]` and returns the value of the `nth` element along `dim`.
fn nth_element(
    samples: &[f32],
    m: usize,
    ids: &mut [u32],
    dim: usize,
    begin: usize,
    end: usize,
    nth: usize,
) -> f32 {
    let val = |ids: &[u32], i: usize| samples[ids[i] as usize * m + dim];
    let nth_index = begin + nth;
    let mut begin_element = begin;
    let mut end_element = end;
    while end_element - begin_element > 3 {
        let b = begin_element;
        let e = end_element - 1;
        let length = end_element - begin_element;
        let middle = begin_element + length / 2;
        let v1 = val(ids, b);
        let v2 = val(ids, e);
        let v3 = val(ids, middle);
        let pivot = median_of_three(v1, v2, v3);
        let cut = unguarded_partition(samples, m, ids, dim, begin_element, end_element, pivot);
        if cut <= nth_index {
            begin_element = cut;
        } else {
            end_element = cut;
        }
    }
    // InsertSort
    for search in begin_element + 1..end_element {
        let mut back = search;
        while back > begin_element {
            if val(ids, back) < val(ids, back - 1) {
                ids.swap(back, back - 1);
            } else {
                break;
            }
            back -= 1;
        }
    }
    val(ids, nth_index)
}

/// `Algorithm::MedianOfThree`.
fn median_of_three(a: f32, b: f32, c: f32) -> f32 {
    if a < b {
        if b < c {
            b
        } else if a < c {
            c
        } else {
            a
        }
    } else if a < c {
        a
    } else if b < c {
        c
    } else {
        b
    }
}

/// `Algorithm::UnguardedPartition`.
fn unguarded_partition(
    samples: &[f32],
    m: usize,
    ids: &mut [u32],
    dim: usize,
    mut begin: usize,
    mut end: usize,
    pivot: f32,
) -> usize {
    let val = |ids: &[u32], i: usize| samples[ids[i] as usize * m + dim];
    loop {
        while val(ids, begin) < pivot {
            begin += 1;
        }
        end -= 1;
        while pivot < val(ids, end) {
            end -= 1;
        }
        if begin >= end {
            return begin;
        }
        ids.swap(begin, end);
        begin += 1;
    }
}

/// `EuclideanDistanceMetric<Array<double>>`.
#[inline]
fn distance64(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0f64;
    for (x, y) in a.iter().zip(b) {
        let t = x - y;
        s += t * t;
    }
    s.sqrt()
}

/// `KdTreeBasedKmeansEstimator`'s settings.
#[derive(Clone, Debug)]
pub struct Kmeans {
    /// `SetMaximumIteration`.
    pub maximum_iteration: usize,
    /// `SetCentroidPositionChangesThreshold`.
    pub threshold: f64,
    /// Count the split instance of every inner node the filter recurses into (larmorx's
    /// correction of ITK-50); `false` drops them, as ITK does.
    pub fix_split_points: bool,
}

/// The result of [`Kmeans::estimate`].
#[derive(Clone, Debug, PartialEq)]
pub struct KmeansResult {
    /// The centres (`K × m`, row-major), as `GetParameters` returns them.
    pub parameters: Vec<f64>,
    /// Iterations made (`GetCurrentIteration`).
    pub iterations: usize,
    /// Whether some pass dropped split instances (ITK-50: only when not fixed) or would have
    /// (when fixed).
    pub split_points_met: bool,
}

struct Candidate {
    centroid: Vec<f64>,
    weighted: Vec<f64>,
    size: i64,
}

impl Kmeans {
    /// `StartOptimization` from the initial centres `initial` (`K × m`, row-major).
    pub fn estimate(&self, tree: &KdTree, initial: &[f64]) -> KmeansResult {
        let m = tree.m;
        let k = initial.len() / m;
        // FindSampleBound over the sample in its own order.
        let n = tree.len();
        let mut lower = vec![0.0f32; m];
        let mut upper = vec![0.0f32; m];
        if n > 0 {
            lower.copy_from_slice(tree.vector(0));
            upper.copy_from_slice(tree.vector(0));
            for id in 1..n as u32 {
                let v = tree.vector(id);
                for d in 0..m {
                    if v[d] < lower[d] {
                        lower[d] = v[d];
                    } else if v[d] > upper[d] {
                        upper[d] = v[d];
                    }
                }
            }
        }
        let mut current: Vec<Vec<f64>> = (0..k)
            .map(|i| initial[i * m..(i + 1) * m].to_vec())
            .collect();
        let valid: Vec<usize> = (0..k).collect();
        let mut iteration = 0usize;
        let mut met = false;
        loop {
            let previous = current.clone();
            let mut candidates: Vec<Candidate> = current
                .iter()
                .map(|c| Candidate {
                    centroid: c.clone(),
                    weighted: vec![0.0; m],
                    size: 0,
                })
                .collect();
            let mut state = FilterState {
                tree,
                candidates: &mut candidates,
                vertex: vec![0.0; m],
                point: vec![0.0; m],
                fix: self.fix_split_points,
                met: false,
            };
            state.filter(
                tree.root,
                valid.clone(),
                &mut lower.clone(),
                &mut upper.clone(),
            );
            met |= state.met;
            for c in candidates.iter_mut() {
                if c.size > 0 {
                    for j in 0..m {
                        c.centroid[j] = c.weighted[j] / c.size as f64;
                    }
                }
            }
            current = candidates.into_iter().map(|c| c.centroid).collect();
            if iteration >= self.maximum_iteration {
                break;
            }
            let changes: f64 = previous
                .iter()
                .zip(&current)
                .map(|(a, b)| distance64(a, b))
                .fold(0.0, |s, d| s + d);
            if changes <= self.threshold {
                break;
            }
            iteration += 1;
        }
        KmeansResult {
            parameters: current.into_iter().flatten().collect(),
            iterations: iteration,
            split_points_met: met,
        }
    }
}

struct FilterState<'t, 'c> {
    tree: &'t KdTree,
    candidates: &'c mut Vec<Candidate>,
    vertex: Vec<f64>,
    point: Vec<f64>,
    fix: bool,
    met: bool,
}

impl FilterState<'_, '_> {
    /// `GetClosestCandidate`: starts from index 0 (not the first valid index) and keeps the
    /// first smallest distance.
    fn closest(&self, x: &[f64], valid: &[usize]) -> usize {
        let mut closest = 0;
        let mut best = f64::MAX;
        for &i in valid {
            let d = distance64(&self.candidates[i].centroid, x);
            if d < best {
                closest = i;
                best = d;
            }
        }
        closest
    }

    /// `IsFarther(a, b, lower, upper)`.
    fn is_farther(&mut self, a: usize, b: &[f64], lower: &[f32], upper: &[f32]) -> bool {
        let pa = &self.candidates[a].centroid;
        for i in 0..self.vertex.len() {
            self.vertex[i] = if pa[i] - b[i] < 0.0 {
                f64::from(lower[i])
            } else {
                f64::from(upper[i])
            };
        }
        distance64(pa, &self.vertex) >= distance64(b, &self.vertex)
    }

    fn add_point(&mut self, id: u32, valid: &[usize]) {
        let v = self.tree.vector(id);
        for (p, &x) in self.point.iter_mut().zip(v) {
            *p = f64::from(x);
        }
        let point = std::mem::take(&mut self.point);
        let c = self.closest(&point, valid);
        for (w, &x) in self.candidates[c].weighted.iter_mut().zip(&point) {
            *w += x;
        }
        self.candidates[c].size += 1;
        self.point = point;
    }

    /// `Filter(node, validIndexes, lower, upper)`; `valid` is taken by value, as in ITK.
    fn filter(&mut self, node: usize, mut valid: Vec<usize>, lower: &mut [f32], upper: &mut [f32]) {
        let tree = self.tree;
        match &tree.nodes[node] {
            Node::Empty => {}
            Node::Leaf(ids) => {
                for &id in ids {
                    self.add_point(id, &valid);
                }
            }
            Node::Inner {
                dim,
                value,
                id,
                left,
                right,
                weighted_centroid,
                centroid,
                size,
            } => {
                let closest = self.closest(centroid, &valid);
                let closest_position = self.candidates[closest].centroid.clone();
                let mut i = 0;
                while i < valid.len() {
                    if valid[i] != closest
                        && self.is_farther(valid[i], &closest_position, lower, upper)
                    {
                        valid.remove(i);
                        continue;
                    }
                    i += 1;
                }
                if valid.len() == 1 {
                    let c = &mut self.candidates[closest];
                    for (w, &x) in c.weighted.iter_mut().zip(weighted_centroid) {
                        *w += x;
                    }
                    c.size += i64::from(*size);
                } else {
                    let (dim, value) = (*dim, *value);
                    // ITK-50: ITK never counts the split instance here.
                    self.met = true;
                    if self.fix {
                        self.add_point(*id, &valid);
                    }
                    let t = upper[dim];
                    upper[dim] = value;
                    self.filter(*left, valid.clone(), lower, upper);
                    upper[dim] = t;
                    let t = lower[dim];
                    lower[dim] = value;
                    self.filter(*right, valid, lower, upper);
                    lower[dim] = t;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> f32 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((*seed >> 40) as f32) / (1u64 << 24) as f32
    }

    #[test]
    fn nn_search_is_exact() {
        let mut s = 7u64;
        let m = 2;
        let data: Vec<f32> = (0..2000 * m).map(|_| lcg(&mut s) * 100.0).collect();
        let tree = KdTree::new(&data, m, 16).unwrap();
        for q in 0..50 {
            let query = [lcg(&mut s) * 100.0, lcg(&mut s) * 100.0];
            let found = tree.search(&query, 7).unwrap();
            let mut all: Vec<(f64, u32)> = (0..2000u32)
                .map(|i| (KdTree::distance(&query, tree.vector(i)), i))
                .collect();
            all.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mut f: Vec<u32> = found.clone();
            f.sort_unstable();
            let mut e: Vec<u32> = all[..7].iter().map(|x| x.1).collect();
            e.sort_unstable();
            assert_eq!(f, e, "query {q}");
        }
        assert!(tree.search(&[0.0, 0.0], 2001).is_err());
    }

    #[test]
    fn nth_element_finds_the_median() {
        let mut s = 3u64;
        for n in [4usize, 5, 17, 100, 333] {
            let data: Vec<f32> = (0..n).map(|_| (lcg(&mut s) * 20.0).floor()).collect();
            let mut ids: Vec<u32> = (0..n as u32).collect();
            let v = nth_element(&data, 1, &mut ids, 0, 0, n, n / 2);
            let mut sorted = data.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(v, sorted[n / 2]);
        }
    }

    #[test]
    fn kmeans_separates_clusters_and_counts_every_point_when_fixed() {
        let mut s = 11u64;
        let mut data = Vec::new();
        for c in [10.0f32, 50.0, 90.0] {
            for _ in 0..3000 {
                data.push(c + lcg(&mut s) * 4.0);
            }
        }
        let tree = KdTree::new(&data, 1, 16).unwrap();
        let km = Kmeans {
            maximum_iteration: 200,
            threshold: 0.0,
            fix_split_points: true,
        };
        let r = km.estimate(&tree, &[20.0, 40.0, 80.0]);
        for (c, want) in r.parameters.iter().zip([12.0, 52.0, 92.0]) {
            assert!((c - want).abs() < 0.2, "{c}");
        }
        let itk = Kmeans {
            fix_split_points: false,
            ..km
        };
        let r2 = itk.estimate(&tree, &[20.0, 40.0, 80.0]);
        assert!(r2.split_points_met);
    }
}
