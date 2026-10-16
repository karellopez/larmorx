// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! The version-1 B-tree that indexes a chunked dataset's chunks, built as HDF5 1.12.1 builds it
//! (`H5B_create`, `H5B_insert`, `H5B__insert_helper`, `H5B__split`, `H5B__insert_child`,
//! `H5B__cache_serialize`, `src/H5B.c`, `src/H5Bcache.c`; the chunk callbacks
//! `H5D__btree_new_node`, `H5D__btree_cmp3`, `H5D__btree_insert`, `H5D__btree_encode_key`,
//! `src/H5Dbtree.c`), with the default K of 32 and split ratios {0.1, 0.5, 0.9}.

use super::alloc::{Mem, Space};
use crate::source::UNDEF;

/// `H5D_btree_key_t`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    nbytes: u32,
    mask: u32,
    scaled: Vec<u64>,
}

#[derive(Clone, Debug)]
struct Node {
    addr: u64,
    level: u8,
    left: u64,
    right: u64,
    /// `2K + 1` key slots (only `nchildren + 1` are meaningful).
    keys: Vec<Key>,
    children: Vec<u64>,
    nchildren: usize,
}

/// A chunk to insert: its scaled coordinates (element dimension included), stored size,
/// filter mask and address.
#[derive(Clone, Debug)]
pub(crate) struct ChunkRecord {
    pub(crate) scaled: Vec<u64>,
    pub(crate) nbytes: u32,
    pub(crate) mask: u32,
    pub(crate) addr: u64,
}

/// A chunk B-tree: its nodes and the chunk dimensions (element size last).
#[derive(Clone, Debug)]
pub(crate) struct ChunkBTree {
    pub(crate) root: u64,
    nodes: Vec<Node>,
    dims: Vec<u32>,
    two_k: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ins {
    Noop,
    Left,
    Right,
    Change,
}

impl ChunkBTree {
    /// `H5B__nodesize` for a chunk B-tree: header, `2K` children and `2K + 1` keys.
    fn node_size(&self) -> usize {
        let key = 4 + 4 + 8 * self.dims.len();
        24 + self.two_k * 8 + (self.two_k + 1) * key
    }

    fn blank_node(&self, addr: u64) -> Node {
        let ndims = self.dims.len();
        Node {
            addr,
            level: 0,
            left: UNDEF,
            right: UNDEF,
            keys: vec![
                Key {
                    nbytes: 0,
                    mask: 0,
                    scaled: vec![0; ndims],
                };
                self.two_k + 1
            ],
            children: vec![0; self.two_k],
            nchildren: 0,
        }
    }

    /// `H5B_create`: an empty root node.
    pub(crate) fn create(space: &mut Space, dims: Vec<u32>) -> ChunkBTree {
        let mut t = ChunkBTree {
            root: 0,
            nodes: Vec::new(),
            dims,
            two_k: 64,
        };
        let addr = space.alloc(Mem::BTree, t.node_size() as u64);
        t.root = addr;
        let n = t.blank_node(addr);
        t.nodes.push(n);
        t
    }

    fn node_index(&self, addr: u64) -> usize {
        self.nodes
            .iter()
            .position(|n| n.addr == addr)
            .expect("B-tree node")
    }

    /// `H5D__btree_cmp3`.
    fn cmp3(&self, lt: &Key, scaled: &[u64], rt: &Key) -> i32 {
        if self.dims.len() == 2 {
            if scaled[0] > rt.scaled[0] || (scaled[0] == rt.scaled[0] && scaled[1] >= rt.scaled[1])
            {
                1
            } else if scaled[0] < lt.scaled[0] {
                -1
            } else {
                0
            }
        } else if !lex_lt(scaled, &rt.scaled) {
            // H5VM_vector_ge_u: lexicographic comparison
            1
        } else if lex_lt(scaled, &lt.scaled) {
            -1
        } else {
            0
        }
    }

    /// `H5B_insert`.
    pub(crate) fn insert(&mut self, space: &mut Space, rec: &ChunkRecord) {
        let ndims = self.dims.len();
        let blank = Key {
            nbytes: 0,
            mask: 0,
            scaled: vec![0; ndims],
        };
        let mut lt_key = blank.clone();
        let mut md_key = blank.clone();
        let mut rt_key = blank;
        let (mut lt_changed, mut rt_changed) = (false, false);
        let mut split: Option<u64> = None;
        let root = self.root;
        let my_ins = self.insert_helper(
            space,
            root,
            &mut lt_key,
            &mut lt_changed,
            &mut md_key,
            rec,
            &mut rt_key,
            &mut rt_changed,
            &mut split,
        );
        if my_ins == Ins::Noop {
            return;
        }
        let split_addr = split.expect("split node");
        let ri = self.node_index(root);
        let level = self.nodes[ri].level;
        if !lt_changed {
            lt_key = self.nodes[ri].keys[0].clone();
        }
        if !rt_changed {
            let si = self.node_index(split_addr);
            rt_key = self.nodes[si].keys[self.nodes[si].nchildren].clone();
        }
        // Move the old root elsewhere; the new root keeps the root's address.
        let old_root_addr = space.alloc(Mem::BTree, self.node_size() as u64);
        let mut new_root = self.nodes[ri].clone();
        self.nodes[ri].addr = old_root_addr;
        let si = self.node_index(split_addr);
        self.nodes[si].left = old_root_addr;
        new_root.left = UNDEF;
        new_root.right = UNDEF;
        new_root.level = level + 1;
        new_root.nchildren = 2;
        new_root.children[0] = old_root_addr;
        new_root.keys[0] = lt_key;
        new_root.children[1] = split_addr;
        new_root.keys[1] = md_key;
        new_root.keys[2] = rt_key;
        new_root.addr = root;
        self.nodes.push(new_root);
    }

    /// `H5B__insert_helper`.
    #[allow(clippy::too_many_arguments)]
    fn insert_helper(
        &mut self,
        space: &mut Space,
        addr: u64,
        lt_key: &mut Key,
        lt_changed: &mut bool,
        md_key: &mut Key,
        rec: &ChunkRecord,
        rt_key: &mut Key,
        rt_changed: &mut bool,
        split_out: &mut Option<u64>,
    ) -> Ins {
        *lt_changed = false;
        *rt_changed = false;
        let bi = self.node_index(addr);
        let (mut lt, mut idx, mut rt) = (0usize, 0usize, self.nodes[bi].nchildren);
        let mut cmp = -1;
        while lt < rt && cmp != 0 {
            idx = (lt + rt) / 2;
            let n = &self.nodes[bi];
            cmp = self.cmp3(&n.keys[idx], &rec.scaled, &n.keys[idx + 1]);
            if cmp < 0 {
                rt = idx;
            } else {
                lt = idx + 1;
            }
        }
        let mut new_child: u64 = UNDEF;
        let mut child_split: Option<u64> = None;
        let my_ins;
        let level = self.nodes[bi].level;
        let nchildren = self.nodes[bi].nchildren;
        if nchildren == 0 {
            // H5D__btree_new_node(H5B_INS_FIRST)
            let n = &mut self.nodes[bi];
            n.children[0] = rec.addr;
            n.keys[0] = Key {
                nbytes: rec.nbytes,
                mask: rec.mask,
                scaled: rec.scaled.clone(),
            };
            n.keys[1] = Key {
                nbytes: 0,
                mask: 0,
                scaled: rec.scaled.iter().map(|s| s + 1).collect(),
            };
            n.nchildren = 1;
            idx = 0;
            my_ins = Ins::Noop;
        } else if cmp < 0 && idx == 0 {
            if level > 0 {
                let child = self.nodes[bi].children[idx];
                let (mut l, mut r) = (
                    self.nodes[bi].keys[idx].clone(),
                    self.nodes[bi].keys[idx + 1].clone(),
                );
                my_ins = self.insert_helper(
                    space,
                    child,
                    &mut l,
                    lt_changed,
                    md_key,
                    rec,
                    &mut r,
                    rt_changed,
                    &mut child_split,
                );
                self.nodes[bi].keys[idx] = l;
                self.nodes[bi].keys[idx + 1] = r;
                new_child = child_split.unwrap_or(UNDEF);
            } else {
                // Not follow_min: a new minimum leaf (H5D__btree_new_node(H5B_INS_LEFT)).
                *md_key = self.nodes[bi].keys[idx].clone();
                let k = Key {
                    nbytes: rec.nbytes,
                    mask: rec.mask,
                    scaled: rec.scaled.clone(),
                };
                self.nodes[bi].keys[idx] = k;
                new_child = rec.addr;
                *lt_changed = true;
                my_ins = Ins::Left;
            }
        } else if cmp > 0 && idx + 1 >= nchildren {
            if level > 0 {
                idx = nchildren - 1;
                let child = self.nodes[bi].children[idx];
                let (mut l, mut r) = (
                    self.nodes[bi].keys[idx].clone(),
                    self.nodes[bi].keys[idx + 1].clone(),
                );
                my_ins = self.insert_helper(
                    space,
                    child,
                    &mut l,
                    lt_changed,
                    md_key,
                    rec,
                    &mut r,
                    rt_changed,
                    &mut child_split,
                );
                self.nodes[bi].keys[idx] = l;
                self.nodes[bi].keys[idx + 1] = r;
                new_child = child_split.unwrap_or(UNDEF);
            } else {
                // Not follow_max: a new maximum leaf (H5D__btree_new_node(H5B_INS_RIGHT)).
                idx = nchildren - 1;
                *md_key = Key {
                    nbytes: rec.nbytes,
                    mask: rec.mask,
                    scaled: rec.scaled.clone(),
                };
                self.nodes[bi].keys[idx + 1] = Key {
                    nbytes: 0,
                    mask: 0,
                    scaled: rec.scaled.iter().map(|s| s + 1).collect(),
                };
                new_child = rec.addr;
                *rt_changed = true;
                my_ins = Ins::Right;
            }
        } else if cmp != 0 {
            unreachable!("HDF5 aborts: no branch to follow");
        } else if level > 0 {
            let child = self.nodes[bi].children[idx];
            let (mut l, mut r) = (
                self.nodes[bi].keys[idx].clone(),
                self.nodes[bi].keys[idx + 1].clone(),
            );
            my_ins = self.insert_helper(
                space,
                child,
                &mut l,
                lt_changed,
                md_key,
                rec,
                &mut r,
                rt_changed,
                &mut child_split,
            );
            self.nodes[bi].keys[idx] = l;
            self.nodes[bi].keys[idx + 1] = r;
            new_child = child_split.unwrap_or(UNDEF);
        } else {
            // H5D__btree_insert on the leaf child.
            let (l, r) = (
                self.nodes[bi].keys[idx].clone(),
                self.nodes[bi].keys[idx + 1].clone(),
            );
            let _ = r;
            if rec.scaled == l.scaled && l.nbytes > 0 {
                if l.nbytes != rec.nbytes {
                    new_child = rec.addr;
                    self.nodes[bi].keys[idx].nbytes = rec.nbytes;
                    self.nodes[bi].keys[idx].mask = rec.mask;
                    *lt_changed = true;
                    my_ins = Ins::Change;
                } else {
                    my_ins = Ins::Noop;
                }
            } else if disjoint(&l.scaled, &rec.scaled) {
                *md_key = Key {
                    nbytes: rec.nbytes,
                    mask: rec.mask,
                    scaled: rec.scaled.clone(),
                };
                new_child = rec.addr;
                my_ins = Ins::Right;
            } else {
                unreachable!("overlapping chunks");
            }
        }
        // Update the left and right keys of this node.
        if *lt_changed {
            if idx > 0 {
                *lt_changed = false;
            } else {
                *lt_key = self.nodes[bi].keys[idx].clone();
            }
        }
        if *rt_changed {
            if idx + 1 < self.nodes[bi].nchildren {
                *rt_changed = false;
            } else {
                *rt_key = self.nodes[bi].keys[idx + 1].clone();
            }
        }
        let mut target = addr;
        if my_ins == Ins::Change {
            self.nodes[bi].children[idx] = new_child;
        } else if my_ins == Ins::Left || my_ins == Ins::Right {
            if self.nodes[bi].nchildren == self.two_k {
                let s = self.split(space, addr, idx);
                *split_out = Some(s);
                let bi = self.node_index(addr);
                if idx >= self.nodes[bi].nchildren {
                    idx -= self.nodes[bi].nchildren;
                    target = s;
                }
            }
            let ti = self.node_index(target);
            insert_child(&mut self.nodes[ti], idx, new_child, my_ins, md_key);
        }
        if let Some(s) = *split_out {
            let si = self.node_index(s);
            *md_key = self.nodes[si].keys[0].clone();
            Ins::Right
        } else {
            Ins::Noop
        }
    }

    /// `H5B__split`: moves the right part of a full node to a new node (returned).
    fn split(&mut self, space: &mut Space, addr: u64, idx: usize) -> u64 {
        let bi = self.node_index(addr);
        let (left, right) = (self.nodes[bi].left, self.nodes[bi].right);
        let ratio = if right == UNDEF {
            0.9
        } else if left == UNDEF {
            0.1
        } else {
            0.5
        };
        let mut nleft = (self.two_k as f64 * ratio) as usize;
        if idx < nleft && nleft == self.two_k {
            nleft -= 1;
        } else if idx >= nleft && nleft == 0 {
            nleft += 1;
        }
        let nright = self.two_k - nleft;
        let new_addr = space.alloc(Mem::BTree, self.node_size() as u64);
        let mut n = self.blank_node(new_addr);
        let bi = self.node_index(addr);
        n.level = self.nodes[bi].level;
        for k in 0..=nright {
            n.keys[k] = self.nodes[bi].keys[nleft + k].clone();
        }
        for k in 0..nright {
            n.children[k] = self.nodes[bi].children[nleft + k];
        }
        n.nchildren = nright;
        self.nodes[bi].nchildren = nleft;
        n.left = addr;
        n.right = right;
        if right != UNDEF {
            let ri = self.node_index(right);
            self.nodes[ri].left = new_addr;
        }
        let bi = self.node_index(addr);
        self.nodes[bi].right = new_addr;
        self.nodes.push(n);
        new_addr
    }

    /// Every node's bytes at its address (`H5B__cache_serialize`).
    pub(crate) fn serialize(&self) -> Vec<(u64, Vec<u8>)> {
        let size = self.node_size();
        self.nodes
            .iter()
            .map(|n| {
                let mut v = Vec::with_capacity(size);
                v.extend_from_slice(b"TREE");
                v.push(1); // chunk B-tree
                v.push(n.level);
                v.extend_from_slice(&(n.nchildren as u16).to_le_bytes());
                v.extend_from_slice(&n.left.to_le_bytes());
                v.extend_from_slice(&n.right.to_le_bytes());
                let key = |v: &mut Vec<u8>, k: &Key| {
                    v.extend_from_slice(&k.nbytes.to_le_bytes());
                    v.extend_from_slice(&k.mask.to_le_bytes());
                    for (s, d) in k.scaled.iter().zip(&self.dims) {
                        v.extend_from_slice(&(s * u64::from(*d)).to_le_bytes());
                    }
                };
                for u in 0..n.nchildren {
                    key(&mut v, &n.keys[u]);
                    v.extend_from_slice(&n.children[u].to_le_bytes());
                }
                if n.nchildren > 0 {
                    key(&mut v, &n.keys[n.nchildren]);
                }
                v.resize(size, 0);
                (n.addr, v)
            })
            .collect()
    }
}

/// `H5VM_vector_lt_u`: lexicographic `a < b`.
fn lex_lt(a: &[u64], b: &[u64]) -> bool {
    for (x, y) in a.iter().zip(b) {
        if x < y {
            return true;
        }
        if x > y {
            return false;
        }
    }
    false
}

/// `H5D__chunk_disjoint`.
fn disjoint(a: &[u64], b: &[u64]) -> bool {
    a.iter().zip(b).any(|(&x, &y)| x < y || y < x)
}

/// `H5B__insert_child`.
fn insert_child(n: &mut Node, idx: usize, child: u64, anchor: Ins, md_key: &Key) {
    let mut idx = idx;
    if idx + 1 == n.nchildren {
        n.keys[idx + 2] = n.keys[idx + 1].clone();
        n.keys[idx + 1] = md_key.clone();
        if anchor == Ins::Right {
            idx += 1;
        } else {
            n.children[idx + 1] = n.children[idx];
        }
    } else {
        for k in (idx + 1..=n.nchildren).rev() {
            n.keys[k + 1] = n.keys[k].clone();
        }
        n.keys[idx + 1] = md_key.clone();
        if anchor == Ins::Right {
            idx += 1;
        }
        for k in (idx..n.nchildren).rev() {
            n.children[k + 1] = n.children[k];
        }
    }
    n.children[idx] = child;
    n.nchildren += 1;
}
