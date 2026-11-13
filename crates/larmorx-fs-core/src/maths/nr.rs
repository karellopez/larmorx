// SPDX-License-Identifier: Apache-2.0
//! Numerical Recipes' 1-based vectors and matrices (spec §14): floats indexed from a lower to an
//! upper bound, in one block (the values unset in FreeSurfer: zeros here, every caller sets
//! them).

use crate::process::{Process, Stop, codes};

/// `vector(nl, nh)`: floats indexed `nl..=nh`.
#[derive(Clone, Debug, PartialEq)]
pub struct NrVector {
    pub nl: i64,
    pub nh: i64,
    data: Vec<f32>,
}

impl NrVector {
    /// Element `i` (0 outside the bounds).
    pub fn get(&self, i: i64) -> f32 {
        if i < self.nl || i > self.nh {
            return 0.0;
        }
        self.data[(i - self.nl) as usize]
    }

    /// Sets element `i` (dropped outside the bounds).
    pub fn set(&mut self, i: i64, v: f32) {
        if i >= self.nl && i <= self.nh {
            self.data[(i - self.nl) as usize] = v;
        }
    }
}

/// `matrix(nrl, nrh, ncl, nch)`: rows `nrl..=nrh` of columns `ncl..=nch`, row after row.
#[derive(Clone, Debug, PartialEq)]
pub struct NrMatrix {
    pub nrl: i64,
    pub nrh: i64,
    pub ncl: i64,
    pub nch: i64,
    data: Vec<f32>,
}

impl NrMatrix {
    fn index(&self, r: i64, c: i64) -> Option<usize> {
        if r < self.nrl || r > self.nrh || c < self.ncl || c > self.nch {
            return None;
        }
        Some(((r - self.nrl) * (self.nch - self.ncl + 1) + (c - self.ncl)) as usize)
    }

    /// Element (r, c) (0 outside the bounds).
    pub fn get(&self, r: i64, c: i64) -> f32 {
        self.index(r, c).map_or(0.0, |i| self.data[i])
    }

    /// Sets element (r, c) (dropped outside the bounds).
    pub fn set(&mut self, r: i64, c: i64, v: f32) {
        if let Some(i) = self.index(r, c) {
            self.data[i] = v;
        }
    }
}

impl Process {
    /// `vector(nl, nh)`; an allocation the machine cannot give ends the program (code -1).
    pub fn nr_vector(&mut self, nl: i64, nh: i64) -> Result<NrVector, Stop> {
        let n = (nh - nl + 1).max(0) as usize;
        let mut data = Vec::new();
        if data.try_reserve_exact(n).is_err() {
            self.error_exit(codes::NO_FILE, format!("could not allocate vector({nl}, {nh})").as_bytes())?;
        }
        data.resize(n, 0.0);
        Ok(NrVector { nl, nh, data })
    }

    /// `matrix(nrl, nrh, ncl, nch)`; an allocation failure ends the program (code -1).
    pub fn nr_matrix(&mut self, nrl: i64, nrh: i64, ncl: i64, nch: i64) -> Result<NrMatrix, Stop> {
        let (rows, cols) = ((nrh - nrl + 1).max(0), (nch - ncl + 1).max(0));
        let n = (rows * cols) as usize;
        let mut data = Vec::new();
        if data.try_reserve_exact(n).is_err() {
            self.error_exit(codes::NO_FILE, format!("could not allocate matrix({rows}, {cols})").as_bytes())?;
        }
        data.resize(n, 0.0);
        Ok(NrMatrix { nrl, nrh, ncl, nch, data })
    }
}
