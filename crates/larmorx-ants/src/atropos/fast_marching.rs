// SPDX-License-Identifier: Apache-2.0
//! ITK's `FastMarchingImageFilter` (ITK v5.4.5,
//! `Modules/Filtering/FastMarching/include/itkFastMarchingImageFilter.hxx`) as Atropos runs it
//! to propagate prior labels along the mask: trial points of value 0, an optional speed image,
//! a stopping value of `FLT_MAX`, `float` output.
//!
//! The trial heap is libstdc++'s `std::priority_queue<AxisNode, std::vector, std::greater>`;
//! its sift-up and sift-down are reproduced exactly, because the order in which nodes of
//! equal value leave the heap decides which neighbours are alive when a value is computed.
//! The axes used for a voxel are sorted with a stable insertion sort (what `std::sort` does
//! for fewer than 16 elements), so equal values keep axis order.

/// A heap node: value, voxel (linear index).
#[derive(Clone, Copy, Debug)]
struct Node {
    value: f32,
    index: usize,
}

/// `std::priority_queue` with `std::greater` (a min-heap), as libstdc++ implements
/// `push_heap` and `pop_heap`.
struct MinHeap {
    c: Vec<Node>,
}

impl MinHeap {
    fn push(&mut self, node: Node) {
        self.c.push(node);
        let mut hole = self.c.len() - 1;
        // __push_heap with comp(parent, value) = parent > value.
        while hole > 0 {
            let parent = (hole - 1) / 2;
            if self.c[parent].value > node.value {
                self.c[hole] = self.c[parent];
                hole = parent;
            } else {
                break;
            }
        }
        self.c[hole] = node;
    }

    fn top(&self) -> Node {
        self.c[0]
    }

    fn pop(&mut self) {
        let len = self.c.len();
        if len > 1 {
            // __pop_heap: the last element is re-inserted from the root.
            let value = self.c[len - 1];
            self.c[len - 1] = self.c[0];
            let n = len - 1;
            // __adjust_heap(first, 0, n, value)
            let mut hole = 0usize;
            let mut second = 0usize;
            while second < (n - 1) / 2 {
                second = 2 * (second + 1);
                if self.c[second].value > self.c[second - 1].value {
                    second -= 1;
                }
                self.c[hole] = self.c[second];
                hole = second;
            }
            if n & 1 == 0 && second == (n - 2) / 2 {
                second = 2 * (second + 1);
                self.c[hole] = self.c[second - 1];
                hole = second - 1;
            }
            // __push_heap(first, hole, 0, value)
            while hole > 0 {
                let parent = (hole - 1) / 2;
                if self.c[parent].value > value.value {
                    self.c[hole] = self.c[parent];
                    hole = parent;
                } else {
                    break;
                }
            }
            self.c[hole] = value;
        }
        self.c.pop();
    }

    fn is_empty(&self) -> bool {
        self.c.is_empty()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Label {
    Far,
    Alive,
    Trial,
    InitialTrial,
}

/// `m_LargeValue`: `float(FLT_MAX / 2.0)`.
pub const LARGE_VALUE: f32 = (f32::MAX as f64 / 2.0) as f32;

/// Why the fast marching failed (an ITK exception).
#[derive(Clone, Debug, PartialEq)]
pub struct FastMarchingError(pub &'static str);

/// Runs the filter on a grid of `size` and `spacing`: `trial` voxels (linear indices, in the
/// order they were inserted) start with value 0; `speed` (if any) is the speed image, else the
/// speed is 1. Returns the arrival times, `LARGE_VALUE` where never reached.
pub fn fast_marching(
    size: &[usize],
    spacing: &[f64],
    speed: Option<&[f32]>,
    trial: &[usize],
) -> Result<Vec<f32>, FastMarchingError> {
    let d = size.len();
    let n: usize = size.iter().product();
    let mut strides = vec![1usize; d];
    for a in 1..d {
        strides[a] = strides[a - 1] * size[a - 1];
    }
    let mut output = vec![LARGE_VALUE; n];
    let mut label = vec![Label::Far; n];
    let mut heap = MinHeap { c: Vec::new() };
    for &i in trial {
        label[i] = Label::InitialTrial;
        output[i] = 0.0;
        heap.push(Node {
            value: 0.0,
            index: i,
        });
    }
    let coord = |i: usize, a: usize| (i / strides[a]) % size[a];
    let stopping = f64::from(f32::MAX);
    let space_factor: Vec<f64> = spacing.iter().map(|&s| (1.0 / s) * (1.0 / s)).collect();
    let mut used: Vec<(f32, usize)> = Vec::with_capacity(d);
    while !heap.is_empty() {
        let node = heap.top();
        heap.pop();
        let current = f64::from(output[node.index]);
        if f64::from(node.value) != current || label[node.index] == Label::Alive {
            continue;
        }
        if current > stopping {
            break;
        }
        label[node.index] = Label::Alive;
        // UpdateNeighbors: per axis, the lower then the upper neighbour. At the lower border
        // the "lower neighbour" is the voxel itself (alive); at the upper border ITK leaves the
        // index where the lower step put it, so the lower neighbour is updated twice.
        for a in 0..d {
            let c = coord(node.index, a);
            let lower = if c > 0 {
                node.index - strides[a]
            } else {
                node.index
            };
            for side in [-1i64, 1] {
                let neighbour = if side < 0 || c + 1 >= size[a] {
                    lower
                } else {
                    node.index + strides[a]
                };
                let l = label[neighbour];
                if l == Label::Alive || l == Label::InitialTrial {
                    continue;
                }
                // UpdateValue
                used.clear();
                for b in 0..d {
                    let cb = coord(neighbour, b);
                    let mut best = (LARGE_VALUE, b);
                    for s in [-1i64, 1] {
                        let nb = if s < 0 {
                            if cb == 0 {
                                continue;
                            }
                            neighbour - strides[b]
                        } else {
                            if cb + 1 >= size[b] {
                                continue;
                            }
                            neighbour + strides[b]
                        };
                        if label[nb] == Label::Alive {
                            let v = output[nb];
                            if best.0 > v {
                                best.0 = v;
                            }
                        }
                    }
                    // stable insertion by value
                    let mut pos = used.len();
                    while pos > 0 && best.0 < used[pos - 1].0 {
                        pos -= 1;
                    }
                    used.insert(pos, best);
                }
                let mut solution = f64::from(LARGE_VALUE);
                let mut aa = 0.0f64;
                let mut bb = 0.0f64;
                let mut cc = match speed {
                    Some(sp) => {
                        let v = f64::from(sp[neighbour]) / 1.0;
                        let inv = 1.0 / v;
                        -1.0 * (inv * inv)
                    }
                    None => -1.0,
                };
                for &(value, axis) in &used {
                    let value = f64::from(value);
                    if solution >= value {
                        let sf = space_factor[axis];
                        aa += sf;
                        bb += value * sf;
                        cc += (value * value) * sf;
                        let discrim = bb * bb - aa * cc;
                        if discrim < 0.0 {
                            return Err(FastMarchingError(
                                "Discriminant of quadratic equation is negative",
                            ));
                        }
                        solution = (discrim.sqrt() + bb) / aa;
                    } else {
                        break;
                    }
                }
                if solution < f64::from(LARGE_VALUE) {
                    let v = solution as f32;
                    output[neighbour] = v;
                    label[neighbour] = Label::Trial;
                    heap.push(Node {
                        value: v,
                        index: neighbour,
                    });
                }
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_pops_in_order() {
        let mut h = MinHeap { c: Vec::new() };
        let vals = [5.0f32, 1.0, 4.0, 1.0, 9.0, 2.0, 6.0, 5.0, 3.0, 5.0];
        for (i, &v) in vals.iter().enumerate() {
            h.push(Node { value: v, index: i });
        }
        let mut out = Vec::new();
        while !h.is_empty() {
            out.push(h.top().value);
            h.pop();
        }
        let mut sorted = vals.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(out, sorted);
    }

    #[test]
    fn distances_on_a_line() {
        let out = fast_marching(&[5], &[2.0], None, &[0]).unwrap();
        assert_eq!(out, [0.0, 2.0, 4.0, 6.0, 8.0]);
        let speed = [1.0f32, 1.0, 0.0, 1.0, 1.0];
        let out = fast_marching(&[5], &[1.0], Some(&speed), &[0]).unwrap();
        assert_eq!(&out[..2], &[0.0, 1.0]);
        assert_eq!(out[2], LARGE_VALUE);
        assert_eq!(out[3], LARGE_VALUE);
    }
}
