// SPDX-License-Identifier: Apache-2.0
//! The constraints between touching regions and their merging (`specs/prelude.md` §4.7, §4.8),
//! built for speed (§10): FSL scans every constraint for each choice (`O(L²)` per volume); here
//! a binary heap keyed by the merit `C` (largest first, then the smallest pair) with lazy
//! deletion gives the same choice sequence in `O(log E)`, each group keeps a list of its
//! neighbours (dead entries skipped), and a weighted union-find holds the integer offsets.
//! Every value is computed in the order and precision the spec fixes; only the structures that
//! find the next choice are larmorx's.

use std::collections::{BTreeSet, BinaryHeap, HashMap};
use std::hash::{BuildHasherDefault, Hasher};

use super::num::{TWO_PI, add32, div64, mul64, narrow, rnd, sub32, sub64, wide};

/// A multiplicative hasher for the pairs' keys (no random state).
#[derive(Default)]
pub struct PairHasher(u64);

impl Hasher for PairHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(b)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (v ^ (v >> 29)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        self.0 ^= self.0 >> 32;
    }
}

/// A map keyed by a pair `a << 32 | b`.
pub type PairMap<V> = HashMap<u64, V, BuildHasherDefault<PairHasher>>;

/// The key of the pair `(a, b)`, `a < b`.
#[inline]
pub fn key(a: u32, b: u32) -> u64 {
    (u64::from(a) << 32) | u64::from(b)
}

#[inline]
fn pair(k: u64) -> (u32, u32) {
    ((k >> 32) as u32, k as u32)
}

/// The sums of §4.7 for one pair while they are collected: `P` (single precision, in the visiting
/// order) and the number of voxel pairs.
#[derive(Clone, Copy, Debug)]
pub struct Sum {
    pub p: f32,
    pub count: u32,
}

impl Sum {
    /// One more voxel pair with the difference `d = fl(phi_a - phi_b)`.
    #[inline]
    pub fn add(&mut self, d: f32) {
        self.p = add32(self.p, d);
        self.count += 1;
    }
}

/// Adds the difference of one voxel pair of regions `l1` (phase `p1`) and `l2` (phase `p2`) to
/// their pair's sums (§4.7: the first pair sets `P`).
#[inline]
pub fn contribute(sums: &mut PairMap<Sum>, l1: u32, p1: f32, l2: u32, p2: f32) {
    let (a, b, d) = if l1 < l2 {
        (l1, l2, sub32(p1, p2))
    } else {
        (l2, l1, sub32(p2, p1))
    };
    sums.entry(key(a, b))
        .and_modify(|s| s.add(d))
        .or_insert(Sum { p: d, count: 1 });
}

/// A voxel-pair count as FSL holds it: a float incremented by one (exact to `2^24`, where adding
/// one no longer changes it).
#[inline]
fn count_f32(c: u32) -> f32 {
    c.min(1 << 24) as f32
}

/// `K = fl(-P / (2 pi N))` and `C = fl(N (0.5 - |fl(rnd(K) - K)|))` (§4.7).
#[inline]
pub fn merit(p: f32, n: f32) -> (f32, f32) {
    let k = narrow(div64(-wide(p), TWO_PI * f64::from(n)));
    let r = rnd(wide(k)) as f32;
    let frac = sub32(r, k).abs();
    let c = narrow(mul64(f64::from(n), sub64(0.5, wide(frac))));
    (k, c)
}

/// One constraint.
#[derive(Clone, Copy, Debug)]
struct Cons {
    p: f32,
    n: f32,
    k: f32,
    stamp: u32,
}

/// A merit as an ordered integer (`-0` and `+0` equal); NaN is kept apart by the caller.
#[inline]
fn ordered(c: f32) -> u32 {
    let b = (c + 0.0).to_bits();
    if b & 0x8000_0000 != 0 {
        !b
    } else {
        b | 0x8000_0000
    }
}

/// A heap entry: the largest merit first, then the smallest pair.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Entry {
    c: u32,
    a: u32,
    b: u32,
    stamp: u32,
}

impl Ord for Entry {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.c
            .cmp(&o.c)
            .then_with(|| o.a.cmp(&self.a))
            .then_with(|| o.b.cmp(&self.b))
            .then_with(|| self.stamp.cmp(&o.stamp))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

/// What the merging gives for one set of regions.
#[derive(Clone, Debug, Default)]
pub struct Merged {
    /// The offset of each region in turns, `o_l` (index 0 unused).
    pub offsets: Vec<f32>,
    /// The number of choices (`Did while loop <count> times`).
    pub choices: usize,
}

/// The constraints of regions `1..=n_regions` from their sums, merged (§4.8). `lines` receives
/// the `<count> constraints left` lines (with `verbose`).
pub fn merge(n_regions: u32, sums: PairMap<Sum>, verbose: bool, lines: &mut Vec<String>) -> Merged {
    let l = n_regions as usize;
    let mut cons: PairMap<Cons> = PairMap::default();
    cons.reserve(sums.len());
    let mut adj: Vec<Vec<u32>> = vec![Vec::new(); l + 1];
    // the totals of §4.8 (exact integers)
    let mut total: Vec<u64> = vec![0; l + 1];
    let mut heap: BinaryHeap<Entry> = BinaryHeap::with_capacity(sums.len());
    let mut nan_pairs: BTreeSet<(u32, u32)> = BTreeSet::new();
    let mut stamp: u32 = 0;
    for (&k, s) in &sums {
        let (a, b) = pair(k);
        let n = count_f32(s.count);
        total[a as usize] += n as u64;
        total[b as usize] += n as u64;
        let (kk, c) = merit(s.p, n);
        stamp += 1;
        cons.insert(
            k,
            Cons {
                p: s.p,
                n,
                k: kk,
                stamp,
            },
        );
        if c.is_nan() {
            nan_pairs.insert((a, b));
        } else {
            heap.push(Entry {
                c: ordered(c),
                a,
                b,
                stamp,
            });
        }
        adj[a as usize].push(b);
        adj[b as usize].push(a);
    }
    drop(sums);
    // every live pair, ordered, once a NaN merit exists (§4.8 step 1's rule)
    let mut all_pairs: Option<BTreeSet<(u32, u32)>> =
        (!nan_pairs.is_empty()).then(|| cons.keys().map(|&k| pair(k)).collect());
    let mut alive = vec![true; l + 1];
    // the weighted union-find: each merged group's name points to the group it joined, with
    // the turns it moved by (relative)
    let mut parent: Vec<u32> = (0..=n_regions).collect();
    let mut moved: Vec<i32> = vec![0; l + 1];
    let mut choices = 0usize;
    loop {
        if cons.is_empty() {
            break;
        }
        // the choice (§4.8 step 1)
        let chosen = {
            let first_nan = match (&all_pairs, nan_pairs.is_empty()) {
                (Some(all), false) => all.first().copied().filter(|p| nan_pairs.contains(p)),
                _ => None,
            };
            match first_nan {
                Some(p) => p,
                None => loop {
                    let Some(top) = heap.pop() else {
                        unreachable!("a live constraint has a merit in the heap or the NaN set")
                    };
                    if cons
                        .get(&key(top.a, top.b))
                        .is_some_and(|c| c.stamp == top.stamp)
                    {
                        break (top.a, top.b);
                    }
                },
            }
        };
        if verbose && choices.is_multiple_of(1000) {
            lines.push(format!("{} constraints left", cons.len()));
        }
        choices += 1;
        let (r, s) = chosen;
        let c = cons
            .remove(&key(r, s))
            .expect("the chosen constraint is live");
        if let Some(all) = &mut all_pairs {
            all.remove(&(r, s));
        }
        nan_pairs.remove(&(r, s));
        let k = rnd(wide(c.k));
        // group s moves by -k turns and joins r (§4.8 step 2)
        parent[s as usize] = r;
        moved[s as usize] = k.wrapping_neg();
        alive[s as usize] = false;
        // fold s's other constraints into r's (step 3)
        let neighbours = std::mem::take(&mut adj[s as usize]);
        for t in neighbours {
            if t == r || !alive[t as usize] {
                continue;
            }
            let kst = key(s.min(t), s.max(t));
            let Some(st) = cons.remove(&kst) else {
                continue;
            };
            if let Some(all) = &mut all_pairs {
                all.remove(&(s.min(t), s.max(t)));
            }
            nan_pairs.remove(&(s.min(t), s.max(t)));
            let p_st = if s < t { st.p } else { -st.p };
            let n_st = st.n;
            let krt = key(r.min(t), r.max(t));
            let (p_rt, n_rt, existed) = match cons.remove(&krt) {
                Some(rt) => (if r < t { rt.p } else { -rt.p }, rt.n, true),
                None => (0.0, 0.0, false),
            };
            let n = n_rt + n_st;
            let sum = wide(add32(p_rt, p_st));
            let shift = mul64(TWO_PI * f64::from(n_st), f64::from(k));
            let q = narrow(sub64(sum, shift));
            let p = if r < t { q } else { -q };
            let (kk, cc) = merit(p, n);
            stamp = stamp.wrapping_add(1);
            cons.insert(krt, Cons { p, n, k: kk, stamp });
            let pr = (r.min(t), r.max(t));
            if cc.is_nan() {
                if all_pairs.is_none() {
                    all_pairs = Some(cons.keys().map(|&k| pair(k)).collect());
                } else if let Some(all) = &mut all_pairs {
                    all.insert(pr);
                }
                nan_pairs.insert(pr);
            } else {
                nan_pairs.remove(&pr);
                if let Some(all) = &mut all_pairs {
                    all.insert(pr);
                }
                heap.push(Entry {
                    c: ordered(cc),
                    a: pr.0,
                    b: pr.1,
                    stamp,
                });
            }
            if !existed {
                adj[r as usize].push(t);
                adj[t as usize].push(r);
            }
        }
    }
    if verbose {
        lines.push(format!("Did while loop {choices} times"));
    }
    // the turns of each region (two's-complement sums along its chain: parents come first)
    let mut d: Vec<i32> = vec![0; l + 1];
    for i in 1..=l {
        let p = parent[i] as usize;
        if p != i {
            d[i] = moved[i].wrapping_add(d[p]);
        }
    }
    // the reference region: the largest total, the smallest number among equal (region 1
    // without constraints)
    let mut reference = 1usize.min(l);
    for i in 1..=l {
        if total[i] > total[reference] {
            reference = i;
        }
    }
    let dref = if l == 0 { 0.0 } else { d[reference] as f32 };
    let offsets = (0..=l)
        .map(|i| if i == 0 { 0.0 } else { d[i] as f32 - dref })
        .collect();
    Merged { offsets, choices }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_regions_one_turn_apart() {
        // region 2 lies one turn above region 1 along 10 voxel pairs
        let mut sums = PairMap::default();
        for _ in 0..10 {
            contribute(&mut sums, 2, 0.1 + std::f32::consts::TAU, 1, 0.1);
        }
        let mut lines = Vec::new();
        let m = merge(2, sums, true, &mut lines);
        assert_eq!(m.choices, 1);
        assert_eq!(m.offsets[1], 0.0);
        assert_eq!(m.offsets[2], -1.0);
        assert_eq!(lines, ["1 constraints left", "Did while loop 1 times"]);
    }

    #[test]
    fn merit_of_a_whole_turn() {
        let (k, c) = merit((-TWO_PI * 4.0) as f32, 4.0);
        assert_eq!(k, 1.0);
        assert_eq!(c, 2.0);
        assert!(ordered(1.0) > ordered(0.5));
        assert_eq!(ordered(-0.0), ordered(0.0));
        assert!(ordered(-1.0) < ordered(0.0));
    }
}
