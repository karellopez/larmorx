// SPDX-License-Identifier: Apache-2.0
//! `--only eiglearn` (development only): reads `larmorx_core::vnl_eigen`'s operand-order sites
//! from VNL's own real eigensystem. On the family `eignan`'s matrices, each site's choice (left
//! or right operand first; where GCC puts a negation) is changed one at a time on an experimental
//! copy of the port (`eigexp.rs`, made by `order/eigen/mkexp.py` with the table mutable), kept when
//! fewer matrices differ from VNL bit for bit, until no change helps; then every site is flipped
//! once more from the final table to count the matrices it decides. Prints the table.

use std::sync::atomic::Ordering;

use larmorx_core::x87::cases::SplitMix64;

use crate::checks_eignan::{cases, official};
use crate::eigexp::{ORDER, SITES, init, real_eigensystem};
use crate::{Lib, Tally};

/// The sites of `neg_div` (three choices); every other site has two.
const NEG_DIV: [usize; 2] = [83, 114];
/// The sites of `neg_mul` (six choices).
const NEG_MUL: [usize; 1] = [142];

fn same(n: usize, m: &[f64], th: &[f64]) -> bool {
    let es = real_eigensystem(m, n).expect("checked");
    if es.ierr != 0 {
        return true;
    }
    let nn = n * n;
    let eq = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());
    let (dre, rest) = th.split_at(n);
    let (dim, rest) = rest.split_at(n);
    let (vre, rest) = rest.split_at(nn);
    let (vim, vreal) = rest.split_at(nn);
    let past = es.notices.iter().any(|x| x.defect.id == "ITK-117");
    let cols = if past { n - 1 } else { n };
    eq(&es.d_re, dre)
        && eq(&es.d_im, dim)
        && eq(&es.v_re, vre)
        && (0..n).all(|r| eq(&es.v_im[r * n..r * n + cols], &vim[r * n..r * n + cols]))
        && (0..n).all(|c| !es.vreal_written[c] || (0..n).all(|r| es.vreal[r * n + c].to_bits() == vreal[r * n + c].to_bits()))
}

pub fn run(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0x1ea7_0e16);
    let (calls, _) = cases(&mut rng, 10_000 * scale as usize);
    let theirs = official(lib, &calls);
    let data: Vec<(usize, &Vec<f64>, Vec<f64>)> =
        calls.iter().zip(theirs).filter_map(|((n, m), th)| th.filter(|v| !v.is_empty()).map(|th| (*n, m, th))).collect();
    init();
    if let Ok(start) = std::env::var("EIGLEARN_START") {
        for (s, v) in start.split(',').enumerate() {
            ORDER[s].store(v.trim().parse().unwrap(), Ordering::Relaxed);
        }
    }
    let differing = || data.iter().filter(|(n, m, th)| !same(*n, m, th)).count();
    let mut best = differing();
    println!("eiglearn: {} matrices, {best} differ with the port's table", data.len());
    loop {
        let mut improved = false;
        for s in 0..SITES {
            let cur = ORDER[s].load(Ordering::Relaxed);
            let choices: &[u8] = if NEG_MUL.contains(&s) {
                &[0, 1, 2, 3, 4, 5]
            } else if NEG_DIV.contains(&s) {
                &[0, 1, 2]
            } else {
                &[0, 1]
            };
            for &c in choices {
                if c == cur {
                    continue;
                }
                ORDER[s].store(c, Ordering::Relaxed);
                let d = differing();
                if d < best {
                    println!("  site {s}: {cur} -> {c}: {best} -> {d} differ");
                    best = d;
                    improved = true;
                    break;
                }
                ORDER[s].store(cur, Ordering::Relaxed);
            }
        }
        if !improved {
            break;
        }
    }
    let table: Vec<u8> = (0..SITES).map(|s| ORDER[s].load(Ordering::Relaxed)).collect();
    // a few of the matrices that still differ, with the entries that differ
    for (n, m, th) in data.iter().filter(|(n, m, th)| !same(*n, m, th)).take(6) {
        let es = real_eigensystem(m, *n).expect("checked");
        let ours: Vec<f64> = es.d_re.iter().chain(&es.d_im).chain(&es.v_re).chain(&es.v_im).copied().collect();
        let names = ["Dre", "Dim", "Vre", "Vim"];
        let mut out = Vec::new();
        for (k, (a, b)) in ours.iter().zip(th.iter()).enumerate() {
            if a.to_bits() != b.to_bits() {
                let (part, idx) = if k < 2 * n { (k / n, k % n) } else { (2 + (k - 2 * n) / (n * n), (k - 2 * n) % (n * n)) };
                out.push(format!("{}[{idx}] {:x}/{:x}", names[part], a.to_bits(), b.to_bits()));
            }
        }
        println!("  still n={n} m={:x?}\n    {}", m.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), out.join(" "));
    }
    let mut decided = Vec::new();
    for s in 0..SITES {
        let cur = table[s];
        let alt = if cur == 0 { 1 } else { 0 };
        ORDER[s].store(alt, Ordering::Relaxed);
        let d = differing();
        ORDER[s].store(cur, Ordering::Relaxed);
        if d != best {
            decided.push(format!("{s}:{}", d - best));
        }
    }
    println!("eiglearn: {best} differ with the table; ORDER = {table:?}");
    println!("eiglearn: sites deciding matrices (site:matrices that change): {}", decided.join(" "));
    t.cases += data.len() as u64;
    t.values += data.len() as u64;
    t.differ += best as u64;
}
