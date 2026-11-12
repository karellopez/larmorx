// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `include/fio.h`'s tab-separated table (`TSV`) as far as `fs.core` brings it (queue item
//! 3.1-22: its fields, `ncols`, `nrows`); its reading and writing (`utils/fio.cpp`) come with
//! `fs.io` (queue item 3.1-26).

/// `TSV`: a header line of column names, then rows of numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tsv {
    pub headers: Vec<String>,
    pub data: Vec<Vec<f64>>,
    pub debug: i32,
}

impl Tsv {
    /// `ncols()`: the number of column names.
    pub fn ncols(&self) -> i32 {
        self.headers.len() as i32
    }

    /// `nrows()`: the number of rows.
    pub fn nrows(&self) -> i32 {
        self.data.len() as i32
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn counts() {
        let t = super::Tsv {
            headers: vec!["a".into(), "b".into()],
            data: vec![vec![1.0, 2.0]],
            debug: 0,
        };
        assert_eq!((t.ncols(), t.nrows()), (2, 1));
    }
}
