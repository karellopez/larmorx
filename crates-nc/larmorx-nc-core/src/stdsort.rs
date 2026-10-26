// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! `std::sort` as GCC's libstdc++ implements it (introsort: median-of-three quicksort with an
//! unguarded partition, heapsort past a depth of 2 log2(n), and a final insertion sort with a
//! threshold of 16), written from the algorithm's published description (the SGI STL's
//! introsort, which libstdc++ keeps, with libstdc++'s pivot step). It is not a stable sort:
//! elements that compare equal end up in an order this exact sequence of comparisons and moves
//! decides, which FSL's mesh sweep (`real_self_intersection`, vertices sorted by x) depends on,
//! and so does the order of NaN values in `calc_bval` (FSL-22, [`sort_f32_checked`]).
//! The SGI STL: Copyright (c) 1994 Hewlett-Packard Company, Copyright (c) 1996 Silicon Graphics
//! Computer Systems, Inc.; their permission notice is in `crates-nc/NOTICE`.

const THRESHOLD: usize = 16;

/// Sorts `v` with `less`, moving elements exactly as libstdc++'s `std::sort` does.
pub fn sort_by<T: Clone>(v: &mut [T], less: impl Fn(&T, &T) -> bool) {
    let n = v.len();
    if n == 0 {
        return;
    }
    let lg = (usize::BITS - 1 - n.leading_zeros()) as usize;
    introsort_loop(v, 0, n, lg * 2, &less);
    final_insertion_sort(v, 0, n, &less);
}

fn introsort_loop<T: Clone>(
    v: &mut [T],
    first: usize,
    mut last: usize,
    mut depth: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    while last - first > THRESHOLD {
        if depth == 0 {
            partial_sort(v, first, last, last, less);
            return;
        }
        depth -= 1;
        let cut = unguarded_partition_pivot(v, first, last, less);
        introsort_loop(v, cut, last, depth, less);
        last = cut;
    }
}

fn move_median_to_first<T: Clone>(
    v: &mut [T],
    result: usize,
    a: usize,
    b: usize,
    c: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    if less(&v[a], &v[b]) {
        if less(&v[b], &v[c]) {
            v.swap(result, b);
        } else if less(&v[a], &v[c]) {
            v.swap(result, c);
        } else {
            v.swap(result, a);
        }
    } else if less(&v[a], &v[c]) {
        v.swap(result, a);
    } else if less(&v[b], &v[c]) {
        v.swap(result, c);
    } else {
        v.swap(result, b);
    }
}

fn unguarded_partition<T: Clone>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    pivot: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> usize {
    loop {
        while less(&v[first], &v[pivot]) {
            first += 1;
        }
        last -= 1;
        while less(&v[pivot], &v[last]) {
            last -= 1;
        }
        if first >= last {
            return first;
        }
        v.swap(first, last);
        first += 1;
    }
}

fn unguarded_partition_pivot<T: Clone>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> usize {
    let mid = first + (last - first) / 2;
    move_median_to_first(v, first, first + 1, mid, last - 1, less);
    unguarded_partition(v, first + 1, last, first, less)
}

fn final_insertion_sort<T: Clone>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    if last - first > THRESHOLD {
        insertion_sort(v, first, first + THRESHOLD, less);
        for i in first + THRESHOLD..last {
            unguarded_linear_insert(v, i, less);
        }
    } else {
        insertion_sort(v, first, last, less);
    }
}

fn insertion_sort<T: Clone>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    if first == last {
        return;
    }
    for i in first + 1..last {
        if less(&v[i], &v[first]) {
            let val = v[i].clone();
            // move_backward(first, i, i + 1)
            for k in (first..i).rev() {
                v[k + 1] = v[k].clone();
            }
            v[first] = val;
        } else {
            unguarded_linear_insert(v, i, less);
        }
    }
}

fn unguarded_linear_insert<T: Clone>(v: &mut [T], mut last: usize, less: &impl Fn(&T, &T) -> bool) {
    let val = v[last].clone();
    let mut next = last - 1;
    while less(&val, &v[next]) {
        v[last] = v[next].clone();
        last = next;
        next -= 1;
    }
    v[last] = val;
}

fn partial_sort<T: Clone>(
    v: &mut [T],
    first: usize,
    middle: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    heap_select(v, first, middle, last, less);
    sort_heap(v, first, middle, less);
}

fn heap_select<T: Clone>(
    v: &mut [T],
    first: usize,
    middle: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    make_heap(v, first, middle, less);
    for i in middle..last {
        if less(&v[i], &v[first]) {
            pop_heap(v, first, middle, i, less);
        }
    }
}

fn make_heap<T: Clone>(v: &mut [T], first: usize, last: usize, less: &impl Fn(&T, &T) -> bool) {
    let len = last - first;
    if len < 2 {
        return;
    }
    let mut parent = (len - 2) / 2;
    loop {
        let value = v[first + parent].clone();
        adjust_heap(v, first, parent, len, value, less);
        if parent == 0 {
            return;
        }
        parent -= 1;
    }
}

fn adjust_heap<T: Clone>(
    v: &mut [T],
    first: usize,
    mut hole: usize,
    len: usize,
    value: T,
    less: &impl Fn(&T, &T) -> bool,
) {
    let top = hole;
    let mut second = hole;
    while second < (len - 1) / 2 {
        second = 2 * (second + 1);
        if less(&v[first + second], &v[first + second - 1]) {
            second -= 1;
        }
        v[first + hole] = v[first + second].clone();
        hole = second;
    }
    if len & 1 == 0 && second == (len - 2) / 2 {
        second = 2 * (second + 1);
        v[first + hole] = v[first + second - 1].clone();
        hole = second - 1;
    }
    push_heap(v, first, hole, top, value, less);
}

fn push_heap<T: Clone>(
    v: &mut [T],
    first: usize,
    mut hole: usize,
    top: usize,
    value: T,
    less: &impl Fn(&T, &T) -> bool,
) {
    let mut parent = hole.wrapping_sub(1) / 2;
    while hole > top && less(&v[first + parent], &value) {
        v[first + hole] = v[first + parent].clone();
        hole = parent;
        parent = hole.wrapping_sub(1) / 2;
    }
    v[first + hole] = value;
}

fn pop_heap<T: Clone>(
    v: &mut [T],
    first: usize,
    last: usize,
    result: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    let value = v[result].clone();
    v[result] = v[first].clone();
    adjust_heap(v, first, 0, last - first, value, less);
}

fn sort_heap<T: Clone>(v: &mut [T], first: usize, mut last: usize, less: &impl Fn(&T, &T) -> bool) {
    while last - first > 1 {
        last -= 1;
        pop_heap(v, first, last, last, less);
    }
}

/// Where libstdc++'s unguarded loops would step outside the vector: C++ then reads (and may
/// write) memory that is not the vector's, so the result cannot be reproduced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outside;

/// `std::sort(v.begin(), v.end())` on floats with `operator<`, moving elements exactly as
/// libstdc++ does ([`sort_by`]'s sequence), for a vector that may hold NaN values. `<` with a
/// NaN is false, which is no strict weak order: libstdc++'s unguarded partition and linear
/// insertion then rely on a guard that may not exist and can run past a subrange (still inside
/// the vector, which this follows with C++'s signed iterator arithmetic) or past the vector's
/// ends (memory C++ does not own: [`Outside`], with `v` partly sorted). Without NaN values the
/// result is any correct sort's.
pub fn sort_f32_checked(v: &mut [f32]) -> Result<(), Outside> {
    let n = v.len() as isize;
    if n == 0 {
        return Ok(());
    }
    let mut c = Checked { v };
    let lg = (usize::BITS - 1 - (n as usize).leading_zeros()) as isize;
    c.introsort_loop(0, n, lg * 2)?;
    c.final_insertion_sort(0, n)
}

/// A vector of floats addressed as C++ iterators (signed offsets from `begin()`), every
/// access checked against its ends.
struct Checked<'a> {
    v: &'a mut [f32],
}

impl Checked<'_> {
    fn at(&self, i: isize) -> Result<f32, Outside> {
        usize::try_from(i)
            .ok()
            .and_then(|i| self.v.get(i).copied())
            .ok_or(Outside)
    }

    fn set(&mut self, i: isize, x: f32) -> Result<(), Outside> {
        let slot = usize::try_from(i)
            .ok()
            .and_then(|i| self.v.get_mut(i))
            .ok_or(Outside)?;
        *slot = x;
        Ok(())
    }

    fn less(&self, a: isize, b: isize) -> Result<bool, Outside> {
        Ok(self.at(a)? < self.at(b)?)
    }

    fn swap(&mut self, a: isize, b: isize) -> Result<(), Outside> {
        let (x, y) = (self.at(a)?, self.at(b)?);
        self.set(a, y)?;
        self.set(b, x)
    }

    fn introsort_loop(
        &mut self,
        first: isize,
        mut last: isize,
        mut depth: isize,
    ) -> Result<(), Outside> {
        while last - first > THRESHOLD as isize {
            if depth == 0 {
                // __partial_sort(first, last, last): __heap_select's loop is empty
                self.make_heap(first, last)?;
                return self.sort_heap(first, last);
            }
            depth -= 1;
            let mid = first + (last - first) / 2;
            self.move_median_to_first(first, first + 1, mid, last - 1)?;
            let cut = self.unguarded_partition(first + 1, last, first)?;
            self.introsort_loop(cut, last, depth)?;
            last = cut;
        }
        Ok(())
    }

    fn move_median_to_first(
        &mut self,
        result: isize,
        a: isize,
        b: isize,
        c: isize,
    ) -> Result<(), Outside> {
        let pick = if self.less(a, b)? {
            if self.less(b, c)? {
                b
            } else if self.less(a, c)? {
                c
            } else {
                a
            }
        } else if self.less(a, c)? {
            a
        } else if self.less(b, c)? {
            c
        } else {
            b
        };
        self.swap(result, pick)
    }

    fn unguarded_partition(
        &mut self,
        mut first: isize,
        mut last: isize,
        pivot: isize,
    ) -> Result<isize, Outside> {
        loop {
            while self.less(first, pivot)? {
                first += 1;
            }
            last -= 1;
            while self.less(pivot, last)? {
                last -= 1;
            }
            if first >= last {
                return Ok(first);
            }
            self.swap(first, last)?;
            first += 1;
        }
    }

    fn final_insertion_sort(&mut self, first: isize, last: isize) -> Result<(), Outside> {
        if last - first > THRESHOLD as isize {
            self.insertion_sort(first, first + THRESHOLD as isize)?;
            for i in first + THRESHOLD as isize..last {
                self.unguarded_linear_insert(i)?;
            }
            Ok(())
        } else {
            self.insertion_sort(first, last)
        }
    }

    fn insertion_sort(&mut self, first: isize, last: isize) -> Result<(), Outside> {
        if first == last {
            return Ok(());
        }
        for i in first + 1..last {
            if self.less(i, first)? {
                let val = self.at(i)?;
                for k in (first..i).rev() {
                    let x = self.at(k)?;
                    self.set(k + 1, x)?;
                }
                self.set(first, val)?;
            } else {
                self.unguarded_linear_insert(i)?;
            }
        }
        Ok(())
    }

    fn unguarded_linear_insert(&mut self, mut last: isize) -> Result<(), Outside> {
        let val = self.at(last)?;
        let mut next = last - 1;
        while val < self.at(next)? {
            let x = self.at(next)?;
            self.set(last, x)?;
            last = next;
            next -= 1;
        }
        self.set(last, val)
    }

    fn make_heap(&mut self, first: isize, last: isize) -> Result<(), Outside> {
        let len = last - first;
        if len < 2 {
            return Ok(());
        }
        let mut parent = (len - 2) / 2;
        loop {
            let value = self.at(first + parent)?;
            self.adjust_heap(first, parent, len, value)?;
            if parent == 0 {
                return Ok(());
            }
            parent -= 1;
        }
    }

    fn adjust_heap(
        &mut self,
        first: isize,
        mut hole: isize,
        len: isize,
        value: f32,
    ) -> Result<(), Outside> {
        let top = hole;
        let mut second = hole;
        while second < (len - 1) / 2 {
            second = 2 * (second + 1);
            if self.less(first + second, first + second - 1)? {
                second -= 1;
            }
            let x = self.at(first + second)?;
            self.set(first + hole, x)?;
            hole = second;
        }
        if len & 1 == 0 && second == (len - 2) / 2 {
            second = 2 * (second + 1);
            let x = self.at(first + second - 1)?;
            self.set(first + hole, x)?;
            hole = second - 1;
        }
        // __push_heap(first, hole, top, value)
        let mut parent = (hole - 1) / 2;
        while hole > top && self.at(first + parent)? < value {
            let x = self.at(first + parent)?;
            self.set(first + hole, x)?;
            hole = parent;
            parent = (hole - 1) / 2;
        }
        self.set(first + hole, value)
    }

    fn sort_heap(&mut self, first: isize, mut last: isize) -> Result<(), Outside> {
        while last - first > 1 {
            last -= 1;
            // __pop_heap(first, last, last)
            let value = self.at(last)?;
            let top = self.at(first)?;
            self.set(last, top)?;
            self.adjust_heap(first, 0, last - first, value)?;
        }
        Ok(())
    }
}

/// `std::nth_element(v.begin(), v.begin() + nth, v.end())` on doubles with `<`, as libstdc++
/// performs it (`__introselect`: median-of-three partitions while more than 3 elements remain,
/// heap selection past a depth of 2 log2(n), then an insertion sort). Its unguarded loops rely on
/// a strict weak order; with NaN values they can step outside the vector, where C++ reads memory
/// that is not the vector's: here they stop at its ends instead, and the result says whether
/// that happened.
pub fn nth_element_f64(v: &mut [f64], nth: usize) -> bool {
    let n = v.len();
    if n == 0 || nth >= n {
        return false;
    }
    let less = |a: &f64, b: &f64| a < b;
    let mut outside = false;
    let (mut first, mut last) = (0usize, n);
    let mut depth = (usize::BITS - 1 - n.leading_zeros()) as usize * 2;
    while last - first > 3 {
        if depth == 0 {
            heap_select(v, first, nth + 1, last, &less);
            v.swap(first, nth);
            return outside;
        }
        depth -= 1;
        let mid = first + (last - first) / 2;
        move_median_to_first(v, first, first + 1, mid, last - 1, &less);
        let cut = guarded_partition(v, first + 1, last, first, &mut outside);
        if cut <= nth {
            first = cut;
        } else {
            last = cut;
        }
    }
    // __insertion_sort(first, last)
    for i in first + 1..last {
        if v[i] < v[first] {
            let val = v[i];
            for k in (first..i).rev() {
                v[k + 1] = v[k];
            }
            v[first] = val;
        } else {
            // __unguarded_linear_insert
            let val = v[i];
            let mut hole = i;
            while val < v[hole - 1] {
                v[hole] = v[hole - 1];
                hole -= 1;
                if hole == 0 {
                    outside = true;
                    break;
                }
            }
            v[hole] = val;
        }
    }
    outside
}

/// `__unguarded_partition(first, last, pivot)` on doubles, stopped at the vector's ends.
fn guarded_partition(
    v: &mut [f64],
    mut first: usize,
    mut last: usize,
    pivot: usize,
    outside: &mut bool,
) -> usize {
    let n = v.len();
    loop {
        while v[first] < v[pivot] {
            first += 1;
            if first >= n {
                *outside = true;
                return n - 1;
            }
        }
        if last == 0 {
            *outside = true;
            return first;
        }
        last -= 1;
        while v[pivot] < v[last] {
            if last == 0 {
                *outside = true;
                break;
            }
            last -= 1;
        }
        if first >= last {
            return first;
        }
        v.swap(first, last);
        first += 1;
        if first >= n {
            *outside = true;
            return n - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nth_element_selects() {
        let mut s = 0x9e3779b97f4a7c15u64;
        for n in [1usize, 2, 3, 4, 5, 17, 100, 5000] {
            let v: Vec<f64> = (0..n)
                .map(|_| {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    (s % 997) as f64
                })
                .collect();
            let mut sorted = v.clone();
            sorted.sort_by(f64::total_cmp);
            for nth in [0, n / 3, n - 1] {
                let mut w = v.clone();
                assert!(!nth_element_f64(&mut w, nth));
                assert_eq!(w[nth], sorted[nth], "{n} {nth}");
            }
        }
        let mut w = vec![3.0, f64::NAN, 1.0, 2.0, f64::NAN, 0.5, 7.0, 6.0];
        let _ = nth_element_f64(&mut w, 5);
    }

    #[test]
    fn sorts_like_any_sort_on_distinct_keys() {
        let mut s = 0x2545f4914f6cdd1du64;
        for n in [0usize, 1, 2, 5, 16, 17, 100, 3000] {
            let mut v: Vec<(u32, usize)> = (0..n)
                .map(|i| {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    ((s % 1000) as u32, i)
                })
                .collect();
            let mut w = v.clone();
            sort_by(&mut v, |a, b| a.0 < b.0);
            w.sort_by_key(|a| a.0);
            let keys: Vec<u32> = v.iter().map(|a| a.0).collect();
            let want: Vec<u32> = w.iter().map(|a| a.0).collect();
            assert_eq!(keys, want, "{n}");
        }
    }

    fn noise(s: &mut u64, n: usize, modulo: u64) -> Vec<f32> {
        (0..n)
            .map(|_| {
                *s ^= *s << 13;
                *s ^= *s >> 7;
                *s ^= *s << 17;
                (*s % modulo) as f32
            })
            .collect()
    }

    #[test]
    fn checked_sort_is_sort_by_without_nan() {
        let mut s = 0x853c49e6748fea9bu64;
        for n in [0usize, 1, 2, 16, 17, 40, 1000, 20000] {
            for modulo in [3u64, 1000] {
                let v = noise(&mut s, n, modulo);
                let mut a = v.clone();
                let mut b = v.clone();
                assert_eq!(sort_f32_checked(&mut a), Ok(()));
                sort_by(&mut b, |x, y| x < y);
                assert_eq!(a, b, "{n} {modulo}");
            }
        }
    }

    #[test]
    fn checked_sort_with_nan() {
        // Inside the vector: NaN values end where libstdc++'s moves put them, not last.
        let mut v = vec![1.0, f32::NAN, 0.5, 2.0];
        assert_eq!(sort_f32_checked(&mut v), Ok(()));
        assert_eq!(&v[..2], &[0.5, 1.0]);
        assert!(v[2].is_nan() && v[3] == 2.0);
        // A NaN first is the insertion's guard: the numbers after it are sorted, it stays.
        let mut v = vec![f32::NAN, 3.0, 2.0, 1.0];
        assert_eq!(sort_f32_checked(&mut v), Ok(()));
        assert!(v[0].is_nan() && v[1..] == [1.0, 2.0, 3.0]);
    }

    /// libstdc++'s own results: `validation/probes/libstdcxx-sort/sort_probe.cc` sorts the same
    /// 768 vectors (sizes 0 to 5000, none to all NaN of both signs, few or many ties) with
    /// `std::sort` and hashes them; GCC 11.4's and 14.3's libstdc++ give this hash (FSL
    /// 6.0.7.17 was built with GCC 13.2), with no read outside any vector.
    #[test]
    fn checked_sort_gives_libstdcxx_order_with_nan() {
        let mut s = 0x2545f4914f6cdd1du64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        let mut h = 0xcbf29ce484222325u64;
        let mut mix = |b: u8| h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
        let (mut cases, mut outside, mut nan_inside) = (0, 0, 0);
        for n in [
            0usize, 1, 2, 3, 5, 16, 17, 18, 31, 32, 33, 64, 100, 257, 1000, 5000,
        ] {
            for f in [0u64, 10, 100, 500, 900, 1000] {
                for m in [3u64, 1000] {
                    for _ in 0..4 {
                        let mut v: Vec<f32> = (0..n)
                            .map(|_| {
                                let r = next();
                                if r % 1000 < f {
                                    f32::from_bits(if (r >> 20) & 1 == 1 {
                                        0xffc00000
                                    } else {
                                        0x7fc00000
                                    })
                                } else {
                                    ((r >> 10) % m) as f32
                                }
                            })
                            .collect();
                        let any_nan = v.iter().any(|x| x.is_nan());
                        if sort_f32_checked(&mut v).is_err() {
                            outside += 1;
                            mix(0xee);
                        } else {
                            nan_inside += usize::from(any_nan);
                            for x in &v {
                                x.to_bits().to_le_bytes().into_iter().for_each(&mut mix);
                            }
                        }
                        mix(0x55);
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!((cases, outside, nan_inside), (768, 0, 495));
        assert_eq!(h, 0x63bfd9ba2c340dd2);
    }
}
