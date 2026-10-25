// SPDX-License-Identifier: Apache-2.0
//! `std::nth_element` as libstdc++ implements it (GCC 14.3's `bits/stl_algo.h` and
//! `bits/stl_heap.h`, the library of the ANTs oracle): introselect with the median of three
//! moved to the front, an unguarded partition, a heap select past the depth limit
//! `2·floor(log2(n))`, and an insertion sort of the last three or fewer, every comparison
//! `operator<`. The element it leaves at `nth` is the C++ program's result where values that
//! compare equal differ in their bits (`-0.0` and `+0.0`): ITK's `MedianImageFilter` takes it.
//!
//! The steps follow the library's code statement by statement (indices for iterators).

/// `__iter_less_iter`: `*a < *b`.
#[inline]
fn less(v: &[f32], a: usize, b: usize) -> bool {
    v[a] < v[b]
}

/// `std::nth_element(v.begin(), v.begin() + nth, v.end())`.
pub fn nth_element(v: &mut [f32], nth: usize) {
    let n = v.len();
    if n == 0 || nth == n {
        return;
    }
    let lg = (usize::BITS - 1 - n.leading_zeros()) as usize;
    introselect(v, 0, nth, n, lg * 2);
}

fn introselect(v: &mut [f32], mut first: usize, nth: usize, mut last: usize, mut depth: usize) {
    while last - first > 3 {
        if depth == 0 {
            heap_select(v, first, nth + 1, last);
            // Place the nth largest element in its final position.
            v.swap(first, nth);
            return;
        }
        depth -= 1;
        let cut = unguarded_partition_pivot(v, first, last);
        if cut <= nth {
            first = cut;
        } else {
            last = cut;
        }
    }
    insertion_sort(v, first, last);
}

fn move_median_to_first(v: &mut [f32], result: usize, a: usize, b: usize, c: usize) {
    if less(v, a, b) {
        if less(v, b, c) {
            v.swap(result, b);
        } else if less(v, a, c) {
            v.swap(result, c);
        } else {
            v.swap(result, a);
        }
    } else if less(v, a, c) {
        v.swap(result, a);
    } else if less(v, b, c) {
        v.swap(result, c);
    } else {
        v.swap(result, b);
    }
}

fn unguarded_partition(v: &mut [f32], mut first: usize, mut last: usize, pivot: usize) -> usize {
    loop {
        while less(v, first, pivot) {
            first += 1;
        }
        last -= 1;
        while less(v, pivot, last) {
            last -= 1;
        }
        if first >= last {
            return first;
        }
        v.swap(first, last);
        first += 1;
    }
}

fn unguarded_partition_pivot(v: &mut [f32], first: usize, last: usize) -> usize {
    let mid = first + (last - first) / 2;
    move_median_to_first(v, first, first + 1, mid, last - 1);
    unguarded_partition(v, first + 1, last, first)
}

fn insertion_sort(v: &mut [f32], first: usize, last: usize) {
    if first == last {
        return;
    }
    for i in first + 1..last {
        if less(v, i, first) {
            let val = v[i];
            v.copy_within(first..i, first + 1);
            v[first] = val;
        } else {
            unguarded_linear_insert(v, i);
        }
    }
}

fn unguarded_linear_insert(v: &mut [f32], mut last: usize) {
    let val = v[last];
    let mut next = last - 1;
    while val < v[next] {
        v[last] = v[next];
        last = next;
        next -= 1;
    }
    v[last] = val;
}

/// `__push_heap(first, hole, top, value)` (`__iter_comp_val`: `*it < value`).
fn push_heap(v: &mut [f32], first: usize, mut hole: usize, top: usize, value: f32) {
    let mut parent = hole.wrapping_sub(1) / 2;
    while hole > top && v[first + parent] < value {
        v[first + hole] = v[first + parent];
        hole = parent;
        parent = hole.wrapping_sub(1) / 2;
    }
    v[first + hole] = value;
}

fn adjust_heap(v: &mut [f32], first: usize, mut hole: usize, len: usize, value: f32) {
    let top = hole;
    let mut second = hole;
    while second < (len - 1) / 2 {
        second = 2 * (second + 1);
        if less(v, first + second, first + (second - 1)) {
            second -= 1;
        }
        v[first + hole] = v[first + second];
        hole = second;
    }
    if len & 1 == 0 && second == (len - 2) / 2 {
        second = 2 * (second + 1);
        v[first + hole] = v[first + (second - 1)];
        hole = second - 1;
    }
    push_heap(v, first, hole, top, value);
}

fn make_heap(v: &mut [f32], first: usize, last: usize) {
    if last - first < 2 {
        return;
    }
    let len = last - first;
    let mut parent = (len - 2) / 2;
    loop {
        let value = v[first + parent];
        adjust_heap(v, first, parent, len, value);
        if parent == 0 {
            return;
        }
        parent -= 1;
    }
}

fn pop_heap(v: &mut [f32], first: usize, last: usize, result: usize) {
    let value = v[result];
    v[result] = v[first];
    adjust_heap(v, first, 0, last - first, value);
}

fn heap_select(v: &mut [f32], first: usize, middle: usize, last: usize) {
    make_heap(v, first, middle);
    for i in middle..last {
        if less(v, i, first) {
            pop_heap(v, first, middle, i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The element at `nth` is the `nth` smallest (in value), the elements before it are not
    /// greater and those after it not smaller, for every size and position, ties and
    /// signed zeros included; deterministic.
    #[test]
    fn selects_as_nth_element_does() {
        let mut state = 12345u64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as u32
        };
        for n in 1..80usize {
            for round in 0..20 {
                let v: Vec<f32> = (0..n)
                    .map(|_| match next() % 7 {
                        0 => 0.0,
                        1 => -0.0,
                        k => (next() % 5) as f32 - 2.0 + k as f32 * 0.0,
                    })
                    .collect();
                let nth = (next() as usize) % n;
                let mut w = v.clone();
                nth_element(&mut w, nth);
                let mut sorted = v.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                assert_eq!(w[nth], sorted[nth], "n {n} round {round}");
                assert!(w[..nth].iter().all(|&x| x <= w[nth]));
                assert!(w[nth + 1..].iter().all(|&x| x >= w[nth]));
                let mut again = v.clone();
                nth_element(&mut again, nth);
                assert_eq!(
                    w.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                    again.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
                );
            }
        }
    }
}
