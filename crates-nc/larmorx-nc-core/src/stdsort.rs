// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! `std::sort` as GCC's libstdc++ implements it (introsort: median-of-three quicksort with an
//! unguarded partition, heapsort past a depth of 2 log2(n), and a final insertion sort with a
//! threshold of 16), written from the algorithm's published description (the SGI STL's
//! introsort, which libstdc++ keeps). It is not a stable sort: elements that compare equal end
//! up in an order this exact sequence of comparisons and moves decides, which FSL's mesh sweep
//! (`real_self_intersection`, vertices sorted by x) depends on.

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
}
