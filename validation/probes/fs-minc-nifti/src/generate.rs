// SPDX-License-Identifier: Apache-2.0
//! Generated `.xfm` files: well-formed ones of every type and form, and mutated ones.

use larmorx_core::x87::cases::SplitMix64;

pub struct Gen {
    pub rng: SplitMix64,
}

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen {
            rng: SplitMix64::new(seed),
        }
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.rng.next_u64() % n
    }

    pub fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len() as u64) as usize]
    }

    /// A double as a test file would hold it.
    pub fn number(&mut self) -> String {
        let v = self.value();
        match self.below(24) {
            0..=9 => larmorx_core::cfmt::g(v, 15),
            10..=12 => larmorx_core::cfmt::g(v, 17),
            13 => larmorx_core::cfmt::g(v, 6),
            14 => format!("{}", v.round() as i64),
            15 => larmorx_core::cfmt::e(v, 10),
            16 => format!("{v:.12}"),
            17 => {
                let b = v.to_bits();
                format!("{}0x1.{:013x}p{}", if b >> 63 == 1 { "-" } else { "" }, b & ((1 << 52) - 1), ((b >> 52) & 0x7ff) as i64 - 1023)
            }
            18 => {
                let t = crate::checks_scan::token(&mut self.rng);
                String::from_utf8_lossy(&t).replace([' ', '\t', ';', '\n'], "")
            }
            _ => larmorx_core::cfmt::g(v, 15),
        }
    }

    pub fn value(&mut self) -> f64 {
        match self.below(6) {
            0 => (self.below(2001) as f64 - 1000.0) / 8.0,
            1 => (self.rng.next_u64() as i64 as f64) / 2f64.powi(60),
            2 => (self.rng.next_u64() as i64 as f64) / 2f64.powi(52) * 128.0,
            3 => [0.0, -0.0, 1.0, -1.0][self.below(4) as usize],
            4 => f64::from_bits(self.rng.next_u64()),
            _ => ((self.rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 300.0,
        }
    }

    /// White space between numbers.
    pub fn sep(&mut self) -> &'static str {
        match self.below(40) {
            0 => "\t",
            1 => "\r\n",
            2 => "\n % a comment\n",
            3 => "  ",
            4 => " # c\n",
            5..=9 => "\n ",
            _ => " ",
        }
    }

    /// A 3 x 4 affine as text (rows of four).
    pub fn matrix(&mut self, n_values: usize) -> String {
        let mut s = String::new();
        let kind = self.below(5);
        for k in 0..n_values {
            if k % 4 == 0 {
                s.push_str(if k == 0 { " " } else { self.sep() });
            } else {
                s.push_str(self.sep());
            }
            let v = match kind {
                0 => {
                    // registration-like: near-identity rotation and scale, a shift
                    let (i, j) = (k / 4, k % 4);
                    if j == 3 {
                        (self.value() * 0.3).clamp(-200.0, 200.0)
                    } else if i == j {
                        1.0 + (self.below(2001) as f64 - 1000.0) / 5000.0
                    } else {
                        (self.below(2001) as f64 - 1000.0) / 7000.0
                    }
                }
                1 => self.below(9) as f64 - 4.0,
                2 => {
                    if k / 4 == 2 && k % 4 < 3 { 0.0 } else { self.value() }
                }
                _ => self.value(),
            };
            let text = if kind == 0 || kind == 1 {
                larmorx_core::cfmt::g(v, if self.below(2) == 0 { 15 } else { 17 })
            } else {
                // a fifth of the values written in other ways
                if self.below(5) == 0 { self.number() } else { larmorx_core::cfmt::g(v, 15) }
            };
            s.push_str(&text);
        }
        s
    }

    pub fn transform(&mut self) -> String {
        let mut s = String::new();
        let ty = match self.below(20) {
            0..=13 => "Linear",
            14 | 15 => "Thin_Plate_Spline_Transform",
            16 => "Grid_Transform",
            _ => self.pick(&["", "Affine", "linear", "\"Linear\"", "Linear ", "Linear\t", "User", "Grid"]),
        };
        s.push_str(&format!("Transform_Type = {ty};\n"));
        if self.below(5) == 0 {
            let f = self.pick(&["True", "False", "True", "Yes", "\"True\"", "", "True "]);
            s.push_str(&format!("Invert_Flag = {f};\n"));
        }
        match ty {
            "Thin_Plate_Spline_Transform" => {
                let nd: i64 = match self.below(12) {
                    0 => 0,
                    1 => -1,
                    2 => 2,
                    3 => 1,
                    _ => 3,
                };
                let np = self.below(5) as i64;
                s.push_str(&format!("Number_Dimensions = {nd};\nPoints ="));
                let k = (np * nd.max(1) + self.below(3) as i64 - 1).max(0);
                for _ in 0..k {
                    s.push_str(self.sep());
                    s.push_str(&self.number());
                }
                s.push_str(";\nDisplacements =");
                let kd = ((np + nd + 1) * nd.max(1) + self.below(3) as i64 - 1).max(0);
                for _ in 0..kd {
                    s.push_str(self.sep());
                    s.push_str(&self.number());
                }
                s.push_str(";\n");
            }
            "Grid_Transform" => {
                let name = self.pick(&["grid_0.mnc", "/abs/vol.mnc", "\"quoted name.mnc\"", "sub/v.mnc", ""]);
                s.push_str(&format!("Displacement_Volume = {name};\n"));
            }
            _ => {
                let key = if self.below(30) == 0 { "Linear_transform" } else { "Linear_Transform" };
                let n = match self.below(30) {
                    0 => 11,
                    1 => 13,
                    _ => 12,
                };
                s.push_str(&format!("{key} =\n{};\n", self.matrix(n)));
            }
        }
        s
    }

    /// A whole file, sometimes mutated or cut.
    pub fn file(&mut self) -> Vec<u8> {
        let mut s = String::new();
        s.push_str(match self.below(30) {
            0 => "  MNI Transform File  \n",
            1 => "\"MNI Transform File\"\n",
            2 => "% before\nMNI Transform File\n",
            3 => "MNI Transform File\r\n",
            4 => "MNI Transform\n",
            5 => "",
            6 => "#x\n\nMNI Transform File",
            _ => "MNI Transform File\n",
        });
        if self.below(2) == 0 {
            s.push_str("%Created by a test.\n");
        }
        s.push('\n');
        let n = match self.below(20) {
            0 => 0,
            1..=3 => 2,
            4 => 3,
            _ => 1,
        };
        for _ in 0..n {
            s.push_str(&self.transform());
        }
        let mut b = s.into_bytes();
        let m = self.below(8);
        if m < 2 {
            const ALPHA: &[u8] = b";= \n\t\r%#\"0123456789.eE+-xXabLinearT\x00\xff";
            for _ in 0..=self.below(3) {
                if b.is_empty() {
                    break;
                }
                let at = self.below(b.len() as u64) as usize;
                match self.below(3) {
                    0 => {
                        b.remove(at);
                    }
                    1 => b.insert(at, ALPHA[self.below(ALPHA.len() as u64) as usize]),
                    _ => b[at] = ALPHA[self.below(ALPHA.len() as u64) as usize],
                }
            }
        } else if m == 2 && !b.is_empty() {
            let at = self.below(b.len() as u64) as usize;
            b.truncate(at);
        }
        b
    }
}
