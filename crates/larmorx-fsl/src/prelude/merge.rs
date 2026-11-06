// SPDX-License-Identifier: Apache-2.0
//! The constraints between touching regions and their merging (`specs/prelude.md` §4.7, §4.8),
//! built for speed (§10): FSL scans every constraint for each choice (`O(L²)` per volume); here
//! the constraints live in slots (a slot reused when a fold moves its constraint from `s` to
//! `r`), an indexed 4-ary heap keyed by the merit `C` (largest first, then the smallest pair)
//! gives the same choice sequence in `O(log E)` with each fold's update in place, each group
//! keeps a list of its slots (the constraint between two groups found by scanning the shorter
//! list, no hashing), and a weighted union-find holds the integer offsets. Every value is
//! computed in the order and precision the spec fixes; only the structures that find the next
//! choice are larmorx's (a test checks them against §4.8 written out as the spec states it).

use std::collections::{BTreeMap, HashMap};
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

/// A removed slot's pair.
const DEAD: u32 = u32::MAX;
/// A slot outside the heap (its merit NaN, or removed).
const OUT: u32 = u32::MAX;

/// One constraint: its pair (`a < b`, group names; `a == DEAD` once removed), `P` (as seen
/// from `a`) and `N`. A slot is reused when a fold moves its constraint from `s` to `r`.
#[derive(Clone, Copy, Debug)]
struct Slot {
    a: u32,
    b: u32,
    p: f32,
    n: f32,
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

/// A heap element: the largest merit first, then the smallest pair (`key` holds the merit and
/// the complement of `a`, `tie` the complement of `b`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Elem {
    key: u64,
    tie: u32,
    id: u32,
}

impl Elem {
    fn of(c: f32, a: u32, b: u32, id: u32) -> Elem {
        Elem {
            key: (u64::from(ordered(c)) << 32) | u64::from(!a),
            tie: !b,
            id,
        }
    }
}

/// An indexed 4-ary max-heap of slots: each slot's position is known, so a fold updates its
/// constraint in place (no stale entries).
struct Heap {
    h: Vec<Elem>,
    pos: Vec<u32>,
}

impl Heap {
    const D: usize = 4;

    fn new(mut h: Vec<Elem>, slots: usize) -> Heap {
        let mut pos = vec![OUT; slots];
        // Floyd's construction
        let n = h.len();
        if n > 1 {
            for i in (0..=(n - 2) / Self::D).rev() {
                Self::down(&mut h, i);
            }
        }
        for (i, e) in h.iter().enumerate() {
            pos[e.id as usize] = i as u32;
        }
        Heap { h, pos }
    }

    /// Sifts `h[i]` down (positions not tracked: the construction only).
    fn down(h: &mut [Elem], mut i: usize) {
        let n = h.len();
        loop {
            let first = Self::D * i + 1;
            if first >= n {
                return;
            }
            let mut best = first;
            for c in first + 1..(first + Self::D).min(n) {
                if h[c] > h[best] {
                    best = c;
                }
            }
            if h[best] <= h[i] {
                return;
            }
            h.swap(i, best);
            i = best;
        }
    }

    #[inline]
    fn place(&mut self, i: usize, e: Elem) {
        self.h[i] = e;
        self.pos[e.id as usize] = i as u32;
    }

    fn sift_up(&mut self, mut i: usize) {
        let e = self.h[i];
        while i > 0 {
            let p = (i - 1) / Self::D;
            if self.h[p] >= e {
                break;
            }
            let pe = self.h[p];
            self.place(i, pe);
            i = p;
        }
        self.place(i, e);
    }

    fn sift_down(&mut self, mut i: usize) {
        let n = self.h.len();
        let e = self.h[i];
        loop {
            let first = Self::D * i + 1;
            if first >= n {
                break;
            }
            let mut best = first;
            for c in first + 1..(first + Self::D).min(n) {
                if self.h[c] > self.h[best] {
                    best = c;
                }
            }
            if self.h[best] <= e {
                break;
            }
            let be = self.h[best];
            self.place(i, be);
            i = best;
        }
        self.place(i, e);
    }

    fn pop(&mut self) -> Option<u32> {
        let top = *self.h.first()?;
        self.pos[top.id as usize] = OUT;
        let last = self.h.pop().expect("not empty");
        if !self.h.is_empty() {
            self.place(0, last);
            self.sift_down(0);
        }
        Some(top.id)
    }

    fn remove(&mut self, id: u32) {
        let i = self.pos[id as usize];
        if i == OUT {
            return;
        }
        self.pos[id as usize] = OUT;
        let i = i as usize;
        let last = self.h.pop().expect("not empty");
        if i < self.h.len() {
            self.place(i, last);
            self.sift_up(i);
            let j = self.pos[last.id as usize] as usize;
            self.sift_down(j);
        }
    }

    /// Puts slot `e.id` in the heap with its new key (in place when it is there).
    fn set(&mut self, e: Elem) {
        let i = self.pos[e.id as usize];
        if i == OUT {
            self.h.push(e);
            let n = self.h.len() - 1;
            self.place(n, e);
            self.sift_up(n);
        } else {
            let i = i as usize;
            let old = self.h[i];
            self.place(i, e);
            if e > old {
                self.sift_up(i);
            } else {
                self.sift_down(i);
            }
        }
    }
}

/// The live pairs once a NaN merit exists (§4.8 step 1's rule): every pair, and the NaN ones,
/// each with its slot.
#[derive(Default)]
struct NanRule {
    all: BTreeMap<(u32, u32), u32>,
    nan: BTreeMap<(u32, u32), u32>,
}

/// What the merging gives for one set of regions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Merged {
    /// The offset of each region in turns, `o_l` (index 0 unused).
    pub offsets: Vec<f32>,
    /// The number of choices (`Did while loop <count> times`).
    pub choices: usize,
}

/// The slot of the live constraint between groups `r` and `t`, by a scan of the shorter of their
/// lists (removed and moved slots dropped from it on the way).
fn find(adj: &mut [Vec<u32>], slots: &[Slot], r: u32, t: u32) -> Option<u32> {
    let (g, other) = if adj[r as usize].len() <= adj[t as usize].len() {
        (r, t)
    } else {
        (t, r)
    };
    let (a, b) = (g.min(other), g.max(other));
    let list = &mut adj[g as usize];
    let mut found = None;
    let mut w = 0;
    for i in 0..list.len() {
        let id = list[i];
        let s = slots[id as usize];
        if s.a == DEAD || (s.a != g && s.b != g) {
            continue;
        }
        if s.a == a && s.b == b {
            found = Some(id);
        }
        list[w] = id;
        w += 1;
    }
    list.truncate(w);
    found
}

/// The constraints of regions `1..=n_regions` from their sums, merged (§4.8). `lines` receives
/// the `<count> constraints left` lines and `Did while loop` (with `verbose`).
pub fn merge(n_regions: u32, sums: PairMap<Sum>, verbose: bool, lines: &mut Vec<String>) -> Merged {
    let l = n_regions as usize;
    let mut adj: Vec<Vec<u32>> = vec![Vec::new(); l + 1];
    // the totals of §4.8 (exact integers)
    let mut total: Vec<u64> = vec![0; l + 1];
    let mut slots: Vec<Slot> = Vec::with_capacity(sums.len());
    let mut elems: Vec<Elem> = Vec::with_capacity(sums.len());
    let mut nan_seen = false;
    for (&k, s) in &sums {
        let (a, b) = pair(k);
        let n = count_f32(s.count);
        total[a as usize] += n as u64;
        total[b as usize] += n as u64;
        let id = slots.len() as u32;
        slots.push(Slot { a, b, p: s.p, n });
        let (_, c) = merit(s.p, n);
        if c.is_nan() {
            nan_seen = true;
        } else {
            elems.push(Elem::of(c, a, b, id));
        }
        adj[a as usize].push(id);
        adj[b as usize].push(id);
    }
    drop(sums);
    let mut heap = Heap::new(elems, slots.len());
    let mut rule: Option<NanRule> = None;
    let start_rule = |slots: &[Slot]| {
        let mut r = NanRule::default();
        for (id, s) in slots.iter().enumerate() {
            if s.a != DEAD {
                r.all.insert((s.a, s.b), id as u32);
                if merit(s.p, s.n).1.is_nan() {
                    r.nan.insert((s.a, s.b), id as u32);
                }
            }
        }
        r
    };
    if nan_seen {
        rule = Some(start_rule(&slots));
    }
    let mut live = slots.len();
    let mut alive = vec![true; l + 1];
    // the weighted union-find: each merged group's name points to the group it joined, with
    // the turns it moved by (relative)
    let mut parent: Vec<u32> = (0..=n_regions).collect();
    let mut moved: Vec<i32> = vec![0; l + 1];
    let mut choices = 0usize;
    while live > 0 {
        // the choice (§4.8 step 1): the smallest pair when its merit is NaN, else the largest
        // merit (the smallest pair among equal)
        let first_nan = rule.as_ref().and_then(|ru| {
            let (p, id) = ru.all.first_key_value()?;
            ru.nan.contains_key(p).then_some(*id)
        });
        let id = match first_nan {
            Some(id) => id,
            None => heap
                .pop()
                .expect("a live constraint is in the heap or has a NaN merit"),
        };
        if verbose && choices.is_multiple_of(1000) {
            lines.push(format!("{live} constraints left"));
        }
        choices += 1;
        let chosen = slots[id as usize];
        let (r, s) = (chosen.a, chosen.b);
        slots[id as usize].a = DEAD;
        live -= 1;
        heap.remove(id);
        if let Some(ru) = &mut rule {
            ru.all.remove(&(r, s));
            ru.nan.remove(&(r, s));
        }
        let k = rnd(wide(merit(chosen.p, chosen.n).0));
        // group s moves by -k turns and joins r (§4.8 step 2)
        parent[s as usize] = r;
        moved[s as usize] = k.wrapping_neg();
        alive[s as usize] = false;
        // fold s's other constraints into r's (step 3)
        let ids = std::mem::take(&mut adj[s as usize]);
        for sid in ids {
            let st = slots[sid as usize];
            if st.a == DEAD {
                continue;
            }
            let t = if st.a == s {
                st.b
            } else if st.b == s {
                st.a
            } else {
                continue;
            };
            debug_assert!(t != r && alive[t as usize]);
            let p_st = if s < t { st.p } else { -st.p };
            let n_st = st.n;
            let rt = find(&mut adj, &slots, r, t);
            let (p_rt, n_rt) = match rt {
                Some(i) => {
                    let x = slots[i as usize];
                    (if r < t { x.p } else { -x.p }, x.n)
                }
                None => (0.0, 0.0),
            };
            let n = n_rt + n_st;
            let sum = wide(add32(p_rt, p_st));
            let shift = mul64(TWO_PI * f64::from(n_st), f64::from(k));
            let q = narrow(sub64(sum, shift));
            let p = if r < t { q } else { -q };
            let (a2, b2) = (r.min(t), r.max(t));
            let (_, c) = merit(p, n);
            if let Some(ru) = &mut rule {
                ru.all.remove(&(st.a, st.b));
                ru.nan.remove(&(st.a, st.b));
            }
            let target = match rt {
                Some(i) => {
                    slots[sid as usize].a = DEAD;
                    heap.remove(sid);
                    live -= 1;
                    i
                }
                None => {
                    adj[r as usize].push(sid);
                    sid
                }
            };
            slots[target as usize] = Slot { a: a2, b: b2, p, n };
            if c.is_nan() {
                heap.remove(target);
                if rule.is_none() {
                    rule = Some(start_rule(&slots));
                }
                if let Some(ru) = &mut rule {
                    ru.all.insert((a2, b2), target);
                    ru.nan.insert((a2, b2), target);
                }
            } else {
                heap.set(Elem::of(c, a2, b2, target));
                if let Some(ru) = &mut rule {
                    ru.all.insert((a2, b2), target);
                    ru.nan.remove(&(a2, b2));
                }
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

    /// §4.8 written out as the spec states it (a scan of every constraint in pair order for each
    /// choice, every region of group s moved): the reference for [`merge`].
    fn by_the_spec(l: u32, sums: &PairMap<Sum>) -> (Merged, Vec<String>) {
        let mut cons: BTreeMap<(u32, u32), (f32, f32)> = BTreeMap::new();
        let mut total = vec![0u64; l as usize + 1];
        for (&k, s) in sums {
            let (a, b) = pair(k);
            let n = count_f32(s.count);
            total[a as usize] += n as u64;
            total[b as usize] += n as u64;
            cons.insert((a, b), (s.p, n));
        }
        let mut group: Vec<u32> = (0..=l).collect();
        let mut d = vec![0i32; l as usize + 1];
        let mut lines = Vec::new();
        let mut choices = 0;
        while !cons.is_empty() {
            let mut best: Option<((u32, u32), f32)> = None;
            for (&pr, &(p, n)) in &cons {
                let c = merit(p, n).1;
                match best {
                    None => best = Some((pr, c)),
                    Some((_, bc)) if c > bc => best = Some((pr, c)),
                    _ => {}
                }
            }
            let ((r, s), _) = best.expect("a constraint");
            if choices % 1000 == 0 {
                lines.push(format!("{} constraints left", cons.len()));
            }
            choices += 1;
            let (p, n) = cons.remove(&(r, s)).expect("live");
            let k = rnd(wide(merit(p, n).0));
            for i in 1..=l as usize {
                if group[i] == s {
                    d[i] = d[i].wrapping_sub(k);
                    group[i] = r;
                }
            }
            let others: Vec<(u32, u32)> = cons
                .keys()
                .copied()
                .filter(|&(a, b)| (a == s || b == s) && a != r && b != r)
                .collect();
            for (a, b) in others {
                let t = if a == s { b } else { a };
                let (pst, nst) = cons.remove(&(a, b)).expect("live");
                let p_st = if s < t { pst } else { -pst };
                let (p_rt, n_rt) = match cons.remove(&(r.min(t), r.max(t))) {
                    Some((x, y)) => (if r < t { x } else { -x }, y),
                    None => (0.0, 0.0),
                };
                let q = narrow(sub64(
                    wide(add32(p_rt, p_st)),
                    mul64(TWO_PI * f64::from(nst), f64::from(k)),
                ));
                cons.insert(
                    (r.min(t), r.max(t)),
                    (if r < t { q } else { -q }, n_rt + nst),
                );
            }
        }
        lines.push(format!("Did while loop {choices} times"));
        let mut reference = 1usize.min(l as usize);
        for i in 1..=l as usize {
            if total[i] > total[reference] {
                reference = i;
            }
        }
        let dref = if l == 0 { 0.0 } else { d[reference] as f32 };
        let offsets = (0..=l as usize)
            .map(|i| if i == 0 { 0.0 } else { d[i] as f32 - dref })
            .collect();
        (Merged { offsets, choices }, lines)
    }

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

    #[test]
    fn the_heap_and_lists_give_the_specs_sequence() {
        // random region graphs (a grid's neighbours and random long links), random sums,
        // some merits tied, some NaN
        let mut seed = 12345u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for case in 0..60 {
            let l = 2 + (next() % 60) as u32;
            let mut sums = PairMap::default();
            for _ in 0..(l * 3) {
                let a = 1 + (next() % u64::from(l)) as u32;
                let b = 1 + (next() % u64::from(l)) as u32;
                if a == b {
                    continue;
                }
                let turns = (next() % 7) as f32 - 3.0;
                let noise = ((next() % 1000) as f32 - 500.0) / 400.0;
                let count = 1 + next() % 5;
                for _ in 0..count {
                    let p2 = if case % 7 == 3 && next() % 50 == 0 {
                        f32::NAN
                    } else {
                        0.2 + turns * std::f32::consts::TAU + noise
                    };
                    contribute(&mut sums, a, 0.2, b, p2);
                }
            }
            let mut lines = Vec::new();
            let fast = merge(l, sums.clone(), true, &mut lines);
            let (want, want_lines) = by_the_spec(l, &sums);
            assert_eq!(fast, want, "case {case}");
            assert_eq!(lines, want_lines, "case {case}");
        }
    }
}
