// SPDX-License-Identifier: Apache-2.0
//! The recipes of the compressor's recorded digest (`tests.rs`), shared with the probe
//! `validation/probes/zlib-deflate` (its `testset` command runs them through zlib itself):
//! seeded inputs from plain integer arithmetic, the stream's parameters and how the input and
//! the output space are handed over, with which flushes.

/// One recipe: deflateInit2's parameters, an optional dictionary, the input, and the calls.
#[derive(Clone, Debug)]
pub struct Recipe {
    pub level: i32,
    pub window_bits: i32,
    pub mem_level: i32,
    pub strategy: i32,
    pub dictionary: Option<Vec<u8>>,
    pub data: Vec<u8>,
    /// `(input bytes added, output space, flush)` per call, then `Z_FINISH` calls with 4,096
    /// bytes of output space until the stream ends.
    pub calls: Vec<(usize, usize, i32)>,
}

/// The number of recipes.
pub const N: u32 = 96;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn data(r: &mut Lcg, kind: u32, len: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(len);
    match kind {
        0 => v.resize(len, 0),
        1 => (0..len).for_each(|_| v.push(r.next() as u8)),
        2 => (0..len).for_each(|_| v.push(r.below(5) as u8)),
        3 => {
            while v.len() < len {
                let run = 1 + r.below(400) as usize;
                let b = r.next() as u8;
                for _ in 0..run.min(len - v.len()) {
                    v.push(b);
                }
            }
        }
        4 => {
            let words: Vec<Vec<u8>> = (0..60)
                .map(|_| {
                    (0..1 + r.below(9))
                        .map(|_| b'a' + r.below(26) as u8)
                        .collect()
                })
                .collect();
            while v.len() < len {
                v.extend_from_slice(&words[r.below(60) as usize]);
                v.push(if r.below(10) == 0 { b'\n' } else { b' ' });
            }
            v.truncate(len);
        }
        5 => {
            while v.len() < len {
                if v.len() > 100 && r.below(3) != 0 {
                    let back = 1 + r.below((v.len() as u32).min(40_000)) as usize;
                    for _ in 0..(3 + r.below(260) as usize).min(len - v.len()) {
                        let b = v[v.len() - back];
                        v.push(b);
                    }
                } else {
                    v.push(r.next() as u8);
                }
            }
        }
        _ => {
            let mut i = 0i64;
            while v.len() < len {
                let (x, y) = (i % 97, (i / 97) % 89);
                let val = (x - 40) * (x - 40) + (y - 30) * (y - 30) + i64::from(r.below(9));
                v.extend_from_slice(&(val as i16).to_le_bytes());
                i += 1;
            }
            v.truncate(len);
        }
    }
    v
}

/// Recipe `i` (`0 <= i < N`).
pub fn recipe(i: u32) -> Recipe {
    let mut r = Lcg(0x5eed_0000 + u64::from(i));
    let level = [-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9][(i % 11) as usize];
    let strategy = [0, 0, 0, 1, 2, 3, 4][(i % 7) as usize];
    let window_bits = [15, 15, 9, 12, -15, 31, 8, 27][(i % 8) as usize];
    let mem_level = [8, 8, 1, 9, 4][(i % 5) as usize];
    let len = match i % 6 {
        0 => 0,
        1 => 1 + r.below(100) as usize,
        2 => 1000 + r.below(9000) as usize,
        3 => 30_000 + r.below(40_000) as usize,
        _ => 100_000 + r.below(150_000) as usize,
    };
    let d = data(&mut r, i % 7, len);
    let dictionary = if i % 9 == 4 && window_bits != 31 && window_bits != 27 {
        let n = r.below(40_000) as usize;
        Some(data(&mut r, (i + 3) % 7, n))
    } else {
        None
    };
    let flushes = [0, 0, 2, 0, 3, 1, 5, 0];
    let mut calls = Vec::new();
    for k in 0..(1 + r.below(12)) {
        let add = match r.below(3) {
            0 => r.below(300) as usize,
            1 => r.below(20_000) as usize,
            _ => r.below(120_000) as usize,
        };
        let space = [1usize, 37, 4096, 8192, 65536][r.below(5) as usize];
        calls.push((add, space, flushes[((k + i) % 8) as usize]));
    }
    Recipe {
        level,
        window_bits,
        mem_level,
        strategy,
        dictionary,
        data: d,
        calls,
    }
}

/// Runs a recipe through a stream given as `deflate(input, output, flush) -> (status, used,
/// made)`: every call's status and counts, and the output bytes.
pub fn drive(
    rec: &Recipe,
    mut deflate: impl FnMut(&[u8], &mut [u8], i32) -> (i32, usize, usize),
) -> (Vec<(i32, usize, usize)>, Vec<u8>) {
    let mut log = Vec::new();
    let mut out = Vec::new();
    let (mut ip, mut avail) = (0usize, 0usize);
    let mut buf = Vec::new();
    let mut call =
        |space: usize, flush: i32, ip: &mut usize, avail: &mut usize, out: &mut Vec<u8>| {
            buf.clear();
            buf.resize(space, 0);
            let r = deflate(&rec.data[*ip..*ip + *avail], &mut buf, flush);
            out.extend_from_slice(&buf[..r.2]);
            *ip += r.1;
            *avail -= r.1;
            r
        };
    for &(add, space, flush) in &rec.calls {
        avail += add.min(rec.data.len() - ip - avail);
        log.push(call(space, flush, &mut ip, &mut avail, &mut out));
    }
    for _ in 0..1_000_000 {
        avail = rec.data.len() - ip;
        let r = call(4096, super::Z_FINISH, &mut ip, &mut avail, &mut out);
        log.push(r);
        if r.0 != super::Z_OK {
            break;
        }
    }
    (log, out)
}

/// FNV-1a over a run's calls and bytes.
pub fn digest(log: &[(i32, usize, usize)], out: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |b: &[u8]| {
        for &x in b {
            h ^= u64::from(x);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    for &(s, u, m) in log {
        eat(&s.to_le_bytes());
        eat(&(u as u64).to_le_bytes());
        eat(&(m as u64).to_le_bytes());
    }
    eat(out);
    h
}

/// The port's digest of recipe `i`.
pub fn port_digest(i: u32) -> u64 {
    let rec = recipe(i);
    let Ok(mut d) = super::Deflate::init2(
        rec.level,
        super::Z_DEFLATED,
        rec.window_bits,
        rec.mem_level,
        rec.strategy,
    ) else {
        return 0;
    };
    if let Some(dict) = &rec.dictionary {
        d.set_dictionary(dict);
    }
    let (log, out) = drive(&rec, |a, b, f| d.deflate(a, b, f));
    digest(&log, &out)
}
