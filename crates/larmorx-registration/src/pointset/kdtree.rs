// SPDX-License-Identifier: Apache-2.0
//! ITK's k-d tree over a point set's `Point<float, D>`s, as `PointsLocator` builds and
//! searches it (ITK v5.4.5: `Modules/Registration/Common/include/itkPointsLocator.hxx`,
//! `Modules/Numerics/Statistics/include/itkKdTreeGenerator.hxx`, `itkKdTree.hxx`,
//! `itkStatisticsAlgorithm.hxx` and `itkEuclideanDistanceMetric.hxx`).
//!
//! The tree is ITK's to the node: buckets of at most 16 points, each nonterminal node cut at
//! the median (`NthElement`, ITK's quickselect with a median of three and an insertion sort)
//! of the dimension of largest spread (the last such dimension on a tie), the median point
//! kept in the node. The k-nearest-neighbour search visits the nodes in ITK's order and keeps
//! ITK's list of neighbours (a farthest neighbour replaced in place), so ties between equally
//! distant points resolve as in ITK. Coordinates are `float`; distances are computed from
//! `float` differences in double, as `EuclideanDistanceMetric<Point<float>>` computes them.

/// `NumericTraits<IdentifierType>::max()`, the identifier of an empty neighbour slot.
pub const NO_POINT: usize = usize::MAX;

/// `m_BucketSize` (`PointsLocator` sets 16).
const BUCKET_SIZE: usize = 16;

#[derive(Clone, Debug)]
enum Node {
    /// `m_EmptyTerminalNode`.
    Empty,
    /// A bucket: the identifiers of its points.
    Terminal(Vec<usize>),
    /// A cut: the dimension and the value, the median point, the two halves.
    Nonterminal {
        dimension: usize,
        value: f32,
        id: usize,
        left: usize,
        right: usize,
    },
}

/// A points locator: the points and the tree built over them.
#[derive(Clone, Debug)]
pub struct KdTree<const D: usize> {
    points: Vec<[f32; D]>,
    nodes: Vec<Node>,
    root: usize,
}

/// ITK's `Subsample`: the instance identifiers in the order the generator permutes them.
struct Subsample<'a, const D: usize> {
    points: &'a [[f32; D]],
    ids: Vec<usize>,
}

impl<const D: usize> Subsample<'_, D> {
    #[inline]
    fn value(&self, i: usize, d: usize) -> f32 {
        self.points[self.ids[i]][d]
    }
}

/// `MedianOfThree`.
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

/// `UnguardedPartition`; `None` where ITK would read past the range (coordinates that are not
/// numbers).
fn unguarded_partition<const D: usize>(
    s: &mut Subsample<'_, D>,
    d: usize,
    begin: usize,
    end: usize,
    pivot: f32,
) -> Option<usize> {
    let (mut b, mut e) = (begin as isize, end as isize);
    let n = s.ids.len() as isize;
    let get = |s: &Subsample<'_, D>, i: isize| -> Option<f32> {
        (0..n).contains(&i).then(|| s.value(i as usize, d))
    };
    loop {
        let mut bv = get(s, b)?;
        while bv < pivot {
            b += 1;
            bv = get(s, b)?;
        }
        e -= 1;
        let mut ev = get(s, e)?;
        while pivot < ev {
            e -= 1;
            ev = get(s, e)?;
        }
        if b >= e {
            return Some(b as usize);
        }
        s.ids.swap(b as usize, e as usize);
        b += 1;
    }
}

/// `InsertSort`.
fn insert_sort<const D: usize>(s: &mut Subsample<'_, D>, d: usize, begin: usize, end: usize) {
    for search in begin + 1..end {
        let mut i = search;
        while i > begin {
            if s.value(i, d) < s.value(i - 1, d) {
                s.ids.swap(i, i - 1);
            } else {
                break;
            }
            i -= 1;
        }
    }
}

/// `NthElement`: partitions `[begin, end)` so that its `nth` element is in place; returns its
/// value.
fn nth_element<const D: usize>(
    s: &mut Subsample<'_, D>,
    d: usize,
    begin: usize,
    end: usize,
    nth: usize,
) -> Option<f32> {
    let nth_index = begin + nth;
    let (mut b, mut e) = (begin, end);
    while e - b > 3 {
        let length = e - b;
        let middle = b + length / 2;
        let v1 = s.value(b, d);
        let v2 = s.value(e - 1, d);
        let v3 = s.value(middle, d);
        let pivot = median_of_three(v1, v2, v3);
        let cut = unguarded_partition(s, d, b, e, pivot)?;
        if cut <= nth_index {
            b = cut;
        } else {
            e = cut;
        }
    }
    insert_sort(s, d, b, e);
    Some(s.value(nth_index, d))
}

/// An error building the tree: ITK's quickselect would read past its range (coordinates that
/// are not numbers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KdTreeError;

struct Generator<'a, const D: usize> {
    s: Subsample<'a, D>,
    nodes: Vec<Node>,
}

impl<const D: usize> Generator<'_, D> {
    fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// `GenerateTreeLoop`.
    fn generate(&mut self, begin: usize, end: usize) -> Result<usize, KdTreeError> {
        if end - begin <= BUCKET_SIZE {
            if end == begin {
                return Ok(0);
            }
            let ids = self.s.ids[begin..end].to_vec();
            return Ok(self.push(Node::Terminal(ids)));
        }
        // FindSampleBoundAndMean: the bounds of [begin, end).
        let mut min = [0f32; D];
        let mut max = [0f32; D];
        for d in 0..D {
            min[d] = self.s.value(begin, d);
            max[d] = min[d];
        }
        for i in begin + 1..end {
            for d in 0..D {
                let v = self.s.value(i, d);
                if v < min[d] {
                    min[d] = v;
                } else if v > max[d] {
                    max[d] = v;
                }
            }
        }
        let mut max_spread = -f32::MAX;
        let mut dimension = 0;
        for d in 0..D {
            let spread = max[d] - min[d];
            if spread >= max_spread {
                max_spread = spread;
                dimension = d;
            }
        }
        let median = (end - begin) / 2;
        let value = nth_element(&mut self.s, dimension, begin, end, median).ok_or(KdTreeError)?;
        let median = median + begin;
        let left = self.generate(begin, median)?;
        let right = self.generate(median + 1, end)?;
        let id = self.s.ids[median];
        Ok(self.push(Node::Nonterminal {
            dimension,
            value,
            id,
            left,
            right,
        }))
    }
}

/// `EuclideanDistanceMetric::Evaluate(x1, x2)`: the `float` differences, squared and summed
/// in double.
#[inline]
pub fn distance<const D: usize>(a: &[f32; D], b: &[f32; D]) -> f64 {
    let mut sum = 0.0f64;
    for d in 0..D {
        let t = f64::from(a[d] - b[d]);
        sum += t * t;
    }
    sum.sqrt()
}

/// `EuclideanDistanceMetric::Evaluate(a, b)` of two coordinates.
#[inline]
fn distance1(a: f32, b: f32) -> f64 {
    f64::from(a - b).abs()
}

/// `KdTree::NearestNeighbors`.
struct Neighbors {
    ids: Vec<usize>,
    distances: Vec<f64>,
    farthest: usize,
}

impl Neighbors {
    fn largest(&self) -> f64 {
        self.distances[self.farthest]
    }

    fn replace_farthest(&mut self, id: usize, distance: f64) {
        self.ids[self.farthest] = id;
        self.distances[self.farthest] = distance;
        // NumericTraits<double>::min(): the smallest positive normal double.
        let mut farthest = f64::MIN_POSITIVE;
        for (i, &d) in self.distances.iter().enumerate() {
            if d > farthest {
                farthest = d;
                self.farthest = i;
            }
        }
    }
}

impl<const D: usize> KdTree<D> {
    /// `PointsLocator::Initialize()` over `points`; `None` where ITK's quickselect would read
    /// past its range. An empty set is the caller's to refuse (ITK throws "The number of
    /// points is 0.").
    pub fn new(points: Vec<[f32; D]>) -> Result<Self, KdTreeError> {
        let n = points.len();
        let (nodes, root) = {
            let mut g = Generator {
                s: Subsample {
                    points: &points,
                    ids: (0..n).collect(),
                },
                nodes: vec![Node::Empty],
            };
            let root = g.generate(0, n)?;
            (g.nodes, root)
        };
        Ok(KdTree {
            points,
            nodes,
            root,
        })
    }

    /// The points.
    pub fn points(&self) -> &[[f32; D]] {
        &self.points
    }

    /// The number of points.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether there are no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// `KdTree::Search(query, k, result)` (`k` at most the number of points): the identifiers
    /// of the `k` nearest points, in the order of ITK's neighbour list.
    pub fn search(&self, query: &[f32; D], k: usize) -> Vec<usize> {
        let mut nn = Neighbors {
            ids: vec![NO_POINT; k],
            distances: vec![f64::MAX; k],
            farthest: 0,
        };
        let lo = (-(f64::from(f32::MAX)).sqrt() / 2.0) as f32;
        let hi = ((f64::from(f32::MAX)).sqrt() / 2.0) as f32;
        let mut lower = [lo; D];
        let mut upper = [hi; D];
        if k > 0 {
            self.search_loop(self.root, query, &mut lower, &mut upper, &mut nn);
        }
        nn.ids
    }

    /// `PointsLocator::FindClosestPoint`.
    pub fn closest(&self, query: &[f32; D]) -> usize {
        self.search(query, 1)[0]
    }

    /// `NearestNeighborSearchLoop`.
    fn search_loop(
        &self,
        node: usize,
        query: &[f32; D],
        lower: &mut [f32; D],
        upper: &mut [f32; D],
        nn: &mut Neighbors,
    ) -> bool {
        match &self.nodes[node] {
            Node::Empty => false,
            Node::Terminal(ids) => {
                for &id in ids {
                    let d = distance(query, &self.points[id]);
                    if d < nn.largest() {
                        nn.replace_farthest(id, d);
                    }
                }
                ball_within_bounds(query, lower, upper, nn.largest())
            }
            &Node::Nonterminal {
                dimension,
                value,
                id,
                left,
                right,
            } => {
                let d = distance(query, &self.points[id]);
                if d < nn.largest() {
                    nn.replace_farthest(id, d);
                }
                if query[dimension] <= value {
                    let temp = upper[dimension];
                    upper[dimension] = value;
                    if self.search_loop(left, query, lower, upper, nn) {
                        return true;
                    }
                    upper[dimension] = temp;
                    let temp = lower[dimension];
                    lower[dimension] = value;
                    if bounds_overlap_ball(query, lower, upper, nn.largest()) {
                        self.search_loop(right, query, lower, upper, nn);
                    }
                    lower[dimension] = temp;
                } else {
                    let temp = lower[dimension];
                    lower[dimension] = value;
                    if self.search_loop(right, query, lower, upper, nn) {
                        return true;
                    }
                    lower[dimension] = temp;
                    let temp = upper[dimension];
                    upper[dimension] = value;
                    if bounds_overlap_ball(query, lower, upper, nn.largest()) {
                        self.search_loop(left, query, lower, upper, nn);
                    }
                    upper[dimension] = temp;
                }
                ball_within_bounds(query, lower, upper, nn.largest())
            }
        }
    }
}

/// `BallWithinBounds`.
fn ball_within_bounds<const D: usize>(
    query: &[f32; D],
    lower: &[f32; D],
    upper: &[f32; D],
    radius: f64,
) -> bool {
    for d in 0..D {
        if distance1(query[d], lower[d]) <= radius || distance1(query[d], upper[d]) <= radius {
            return false;
        }
    }
    true
}

/// `BoundsOverlapBall`.
fn bounds_overlap_ball<const D: usize>(
    query: &[f32; D],
    lower: &[f32; D],
    upper: &[f32; D],
    radius: f64,
) -> bool {
    let squared = radius * radius;
    let mut sum = 0.0f64;
    for d in 0..D {
        if query[d] <= lower[d] {
            let t = distance1(query[d], lower[d]);
            sum += t * t;
            if sum < squared {
                return true;
            }
        } else if query[d] >= upper[d] {
            let t = distance1(query[d], upper[d]);
            sum += t * t;
            if sum < squared {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> f32 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((*seed >> 40) as f32) / ((1u64 << 24) as f32) * 50.0
    }

    #[test]
    fn the_nearest_neighbours_are_the_brute_force_ones() {
        let mut seed = 3;
        let pts: Vec<[f32; 3]> = (0..500)
            .map(|_| [lcg(&mut seed), lcg(&mut seed), lcg(&mut seed)])
            .collect();
        let tree = KdTree::new(pts.clone()).unwrap();
        for q in 0..50 {
            let query = [lcg(&mut seed), lcg(&mut seed), lcg(&mut seed)];
            let mut found = tree.search(&query, 7);
            let mut brute: Vec<(f64, usize)> = pts
                .iter()
                .enumerate()
                .map(|(i, p)| (distance(&query, p), i))
                .collect();
            brute.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mut expected: Vec<usize> = brute[..7].iter().map(|x| x.1).collect();
            found.sort_unstable();
            expected.sort_unstable();
            assert_eq!(found, expected, "query {q}");
            assert_eq!(tree.closest(&query), brute[0].1);
        }
    }

    #[test]
    fn small_sets_are_one_bucket() {
        let pts = vec![[0.0f32, 0.0], [1.0, 0.0], [0.0, 2.0]];
        let tree = KdTree::new(pts).unwrap();
        assert_eq!(tree.search(&[0.9, 0.1], 3), vec![0, 1, 2]);
        assert_eq!(tree.search(&[0.1, 0.1], 2).len(), 2);
    }
}
