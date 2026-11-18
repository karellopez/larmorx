// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer 8.2.0's `include/csv.h` (queue item 3.2-04): `readCSVRow` and `readCSV`, the CSV
//! reader of `mri_convert --in_stats_table` (`utils/stats.cpp`'s `LoadStatTableCSV`).
//!
//! A row is split at commas outside double quotes; a double quote opens a quoted part anywhere in a
//! field (`a"b,c",d` is the fields `ab,c` and `d`), `""` inside one is a quote, and a closing quote
//! followed by anything but a comma or a quote ends the quoted part and drops that character
//! (`"a"b` and `a"b,c"d` are `a` and `ab,c`). Nothing else is removed: blanks and a carriage return stay in their field. The
//! file is read line by line (`std::getline`, the newline removed); empty lines are skipped; a last
//! line without a newline is read; a file that cannot be read gives no rows.

/// `CSVState`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    UnquotedField,
    QuotedField,
    QuotedQuote,
}

/// `readCSVRow(row)`: the fields of one line (at least one, the empty field of an empty line).
pub fn read_csv_row(row: &[u8]) -> Vec<Vec<u8>> {
    let mut state = State::UnquotedField;
    let mut fields: Vec<Vec<u8>> = vec![Vec::new()];
    let mut i = 0;
    for &c in row {
        match state {
            State::UnquotedField => match c {
                b',' => {
                    fields.push(Vec::new());
                    i += 1;
                }
                b'"' => state = State::QuotedField,
                _ => fields[i].push(c),
            },
            State::QuotedField => match c {
                b'"' => state = State::QuotedQuote,
                _ => fields[i].push(c),
            },
            State::QuotedQuote => match c {
                b',' => {
                    fields.push(Vec::new());
                    i += 1;
                    state = State::UnquotedField;
                }
                b'"' => {
                    fields[i].push(b'"');
                    state = State::QuotedField;
                }
                _ => state = State::UnquotedField,
            },
        }
    }
    fields
}

/// `readCSV(in)` of a file's bytes: each line's fields (`readCSVRow`), empty lines left out.
pub fn read_csv(bytes: &[u8]) -> Vec<Vec<Vec<u8>>> {
    let mut table = Vec::new();
    let mut rest = bytes;
    // `while (!in.eof())`: a `getline` that extracts nothing at the end fails and stops the loop;
    // one that reaches the end without a newline reads its line and sets the end
    while !rest.is_empty() {
        let (row, next) = match rest.iter().position(|&c| c == b'\n') {
            Some(n) => (&rest[..n], &rest[n + 1..]),
            None => (rest, &rest[rest.len()..]),
        };
        rest = next;
        let fields = read_csv_row(row);
        if !row.is_empty() {
            table.push(fields);
        }
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(row: &str) -> Vec<String> {
        read_csv_row(row.as_bytes())
            .into_iter()
            .map(|b| String::from_utf8(b).unwrap())
            .collect()
    }

    #[test]
    fn rows_and_quotes() {
        assert_eq!(f(""), [""]);
        assert_eq!(f("a,b,,c"), ["a", "b", "", "c"]);
        assert_eq!(f("a\"b,c\"d"), ["ab,c"]);
        assert_eq!(f("a\"b,c\",d"), ["ab,c", "d"]);
        assert_eq!(f("\"a\"\"b\",c"), ["a\"b", "c"]);
        assert_eq!(f("\"a\"b,c"), ["a", "c"]);
        assert_eq!(f(" x ,y\r"), [" x ", "y\r"]);
        assert_eq!(f("\"open,end"), ["open,end"]);
    }

    #[test]
    fn lines() {
        let t = read_csv(b"m,a\n\nr1,1\nr2,2");
        assert_eq!(t.len(), 3);
        assert_eq!(t[2], vec![b"r2".to_vec(), b"2".to_vec()]);
        assert!(read_csv(b"").is_empty());
        assert_eq!(read_csv(b"\n\n").len(), 0);
        assert_eq!(read_csv(b"x\n").len(), 1);
    }
}
