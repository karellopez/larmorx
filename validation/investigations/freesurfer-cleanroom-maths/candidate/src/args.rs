// SPDX-License-Identifier: Apache-2.0
//! The probe's encodings (README.md, "Encodings"): a line's arguments and its printed result.

use larmorx_fs_core::maths::Returned;
use larmorx_fs_core::maths::dmatrix::DMatrix;
use larmorx_fs_core::maths::histo::Histogram;
use larmorx_fs_core::maths::matrix::Matrix;
use larmorx_fs_core::{Process, Stop};

/// Where an output argument points: none given, the first input, the second, or an object.
pub enum Out<T> {
    New,
    First,
    Second,
    Obj(T),
}

/// One line's arguments and result.
pub struct Line {
    toks: Vec<String>,
    pos: usize,
    res: Vec<String>,
}

/// C's `strtol(s, NULL, 10)`.
pub fn strtol(s: &str) -> i64 {
    let b = s.as_bytes();
    let mut i = 0;
    let neg = match b.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };
    let mut v: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add(i64::from(b[i] - b'0'));
        i += 1;
    }
    if neg { -v } else { v }
}

/// C's `strtoull(s, NULL, 16)`.
pub fn strtoul16(s: &str) -> u64 {
    let d: String = s.chars().take_while(char::is_ascii_hexdigit).collect();
    u64::from_str_radix(&d, 16).unwrap_or(u64::MAX)
}

impl Line {
    pub fn new(toks: Vec<String>) -> Line {
        Line {
            toks,
            pos: 0,
            res: Vec::new(),
        }
    }

    pub fn tok(&mut self) -> &str {
        if self.pos >= self.toks.len() {
            eprintln!("probe: missing argument");
            std::process::exit(3);
        }
        self.pos += 1;
        &self.toks[self.pos - 1]
    }

    pub fn peek(&self, s: &str) -> bool {
        self.toks.get(self.pos).is_some_and(|t| t == s)
    }

    pub fn geti(&mut self) -> i32 {
        strtol(self.tok()) as i32
    }

    pub fn getl(&mut self) -> i64 {
        strtol(self.tok())
    }

    pub fn getf(&mut self) -> f32 {
        f32::from_bits(strtoul16(self.tok()) as u32)
    }

    pub fn getd(&mut self) -> f64 {
        f64::from_bits(strtoul16(self.tok()))
    }

    pub fn getx(&mut self) -> Vec<u8> {
        let t = self.tok().to_owned();
        if !t.starts_with('x') {
            eprintln!("probe: a string must start with x");
            std::process::exit(3);
        }
        let b = t.as_bytes();
        let mut s = Vec::new();
        let mut i = 1;
        while i + 1 < b.len() {
            s.push(u8::from_str_radix(&t[i..i + 2], 16).unwrap_or(0));
            i += 2;
        }
        s
    }

    // ---- results

    pub fn out(&mut self, s: impl Into<String>) {
        self.res.push(s.into());
    }

    pub fn puti(&mut self, v: i64) {
        self.out(v.to_string());
    }

    pub fn putf(&mut self, f: f32) {
        self.out(format!("{:08x}", f.to_bits()));
    }

    pub fn putd(&mut self, d: f64) {
        self.out(format!("{:016x}", d.to_bits()));
    }

    pub fn result(&self) -> String {
        self.res.join(" ")
    }

    // ---- histograms

    /// `<alloc> <nbins> <bin_size> <min> <max> <bins> <counts>` or `null`.
    pub fn geth(&mut self, p: &mut Process) -> Result<Option<Histogram>, Stop> {
        if self.peek("null") {
            self.pos += 1;
            return Ok(None);
        }
        let alloc = self.geti();
        let nbins = self.geti();
        let mut h = p.histo_alloc(alloc)?;
        h.nbins = nbins;
        h.bin_size = self.getf();
        h.min = self.getf();
        h.max = self.getf();
        for b in 0..nbins.max(0) {
            let v = self.getf();
            h.set_bin(b, v);
        }
        for b in 0..nbins.max(0) {
            let v = self.getf();
            h.set_count(b, v);
        }
        Ok(Some(h))
    }

    pub fn geth_out(&mut self, p: &mut Process) -> Result<Out<Option<Histogram>>, Stop> {
        Ok(if self.peek("new") {
            self.pos += 1;
            Out::New
        } else if self.peek("self") {
            self.pos += 1;
            Out::First
        } else if self.peek("other") {
            self.pos += 1;
            Out::Second
        } else {
            Out::Obj(self.geth(p)?)
        })
    }

    pub fn puth(&mut self, h: Option<&Histogram>) {
        let Some(h) = h else {
            self.out("null");
            return;
        };
        self.puti(i64::from(h.nbins));
        self.puti(i64::from(h.max_bins));
        self.putf(h.bin_size);
        self.putf(h.min);
        self.putf(h.max);
        for b in 0..h.nbins.max(0) {
            self.putf(h.bin(b));
        }
        for b in 0..h.nbins.max(0) {
            self.putf(h.count(b));
        }
    }

    /// The returned histogram (`given` when it is the output argument), then the given output's
    /// state.
    pub fn puth_res(&mut self, r: &Returned<Histogram>, given: Option<&Histogram>) {
        match r {
            Returned::Output if given.is_some() => self.out("given"),
            Returned::New(h) => self.puth(Some(h)),
            _ => self.out("null"),
        }
        if let Some(g) = given {
            self.puth(Some(g));
        }
    }

    // ---- float matrices

    /// `<type> <rows> <cols> <numbers>` or `null`.
    pub fn getm(&mut self, p: &mut Process) -> Result<Option<Matrix>, Stop> {
        if self.peek("null") {
            self.pos += 1;
            return Ok(None);
        }
        let kind = self.geti();
        let rows = self.geti();
        let cols = self.geti();
        let Some(mut m) = p.matrix_alloc(rows, cols, kind)? else {
            eprintln!("probe: a matrix argument the allocator refused");
            return Err(Stop::Exit(3));
        };
        for i in 0..m.numbers() {
            let v = self.getf();
            m.write(Some(2 + i as i64), v);
        }
        Ok(Some(m))
    }

    pub fn getm_out(&mut self, p: &mut Process) -> Result<Out<Option<Matrix>>, Stop> {
        Ok(if self.peek("new") {
            self.pos += 1;
            Out::New
        } else if self.peek("self") {
            self.pos += 1;
            Out::First
        } else if self.peek("other") {
            self.pos += 1;
            Out::Second
        } else {
            Out::Obj(self.getm(p)?)
        })
    }

    pub fn putm(&mut self, m: Option<&Matrix>) {
        let Some(m) = m else {
            self.out("null");
            return;
        };
        self.puti(i64::from(m.kind));
        self.puti(i64::from(m.rows));
        self.puti(i64::from(m.cols));
        for i in 0..m.numbers() + 2 {
            self.putf(m.read(Some(i as i64)));
        }
    }

    pub fn putm_res(&mut self, r: &Returned<Matrix>, given: Option<&Matrix>) {
        match r {
            Returned::Output if given.is_some() => self.out("given"),
            Returned::New(m) => self.putm(Some(m)),
            _ => self.out("null"),
        }
        if let Some(g) = given {
            self.putm(Some(g));
        }
    }

    // ---- double matrices

    pub fn getdm(&mut self, p: &mut Process) -> Result<Option<DMatrix>, Stop> {
        if self.peek("null") {
            self.pos += 1;
            return Ok(None);
        }
        let rows = self.geti();
        let cols = self.geti();
        let Some(mut m) = p.dmatrix_alloc(rows, cols, 1)? else {
            eprintln!("probe: a double matrix argument the allocator refused");
            return Err(Stop::Exit(3));
        };
        for i in 0..m.data.len() {
            m.data[i] = self.getd();
        }
        Ok(Some(m))
    }

    pub fn getdm_out(&mut self, p: &mut Process) -> Result<Out<Option<DMatrix>>, Stop> {
        Ok(if self.peek("new") {
            self.pos += 1;
            Out::New
        } else if self.peek("self") {
            self.pos += 1;
            Out::First
        } else if self.peek("other") {
            self.pos += 1;
            Out::Second
        } else {
            Out::Obj(self.getdm(p)?)
        })
    }

    pub fn putdm(&mut self, m: Option<&DMatrix>) {
        let Some(m) = m else {
            self.out("null");
            return;
        };
        self.puti(i64::from(m.rows));
        self.puti(i64::from(m.cols));
        for &d in &m.data {
            self.putd(d);
        }
    }

    pub fn putdm_res(&mut self, r: &Returned<DMatrix>, given: Option<&DMatrix>) {
        match r {
            Returned::Output if given.is_some() => self.out("given"),
            Returned::New(m) => self.putdm(Some(m)),
            _ => self.out("null"),
        }
        if let Some(g) = given {
            self.putdm(Some(g));
        }
    }
}
