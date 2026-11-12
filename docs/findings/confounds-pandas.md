# fMRIPrep's confounds: pandas' reading and writing of tables

fMRIPrep's confounds chain reads and writes its tables with pandas 4 to 5 times (the motion table into
`FramewiseDisplacement`, `GatherConfounds`, `ExpandModel`, `SpikeRegressors`, `TSV2JSON`). What porting
that text layer without pandas showed, with fMRIPrep's and niworkflows' table steps on it and the
defects found (PD-01 to PD-03, PIPE-61 to PIPE-63). pandas is BSD-3-Clause, fMRIPrep and niworkflows
Apache-2.0: not restricted. The port: `crates/larmorx-confounds/src/pandas.rs` (pandas 3.0.6's and
2.2.3's `precise_xstrtod`, `try_double`), `python/larmorx/confounds/_tables.py` (the reading, the
writing, ujson's formatting), `python/larmorx/confounds/fmriprep.py`, `python/larmorx/niworkflows/utility.py`
([docs/api/confounds.md](../api/confounds.md), [docs/api/niworkflows-helpers.md](../api/niworkflows-helpers.md)).
The scripts: `check_tables.py`, `probe_pandas_parse.py`
([validation/investigations/confounds](../../validation/investigations/confounds/README.md)).

## pandas' float parser is not correctly rounded (PD-01, rule A)

- **What** (read, verified): pandas' default C-engine float parser (`precise_xstrtod`, pandas 3.0.6 and
  2.2.3, `_libs/src/parser/tokenizer.c`) keeps at most 17 digit characters, the integer part's `0` and
  the decimals' leading zeros of `0.00...` counted, accumulates `number · 10 + digit` in a double and
  divides by one power of ten (the compiler's correctly rounded literal), so most texts `to_csv` writes
  (a float64's shortest `repr`, up to 17 significant digits) are read back as another double, one or
  more units in the last place off.
- **Measured** (`check_tables.py`, section `parse`, 20,000 values per family; `probe_pandas_parse.py`
  before): 54,799 of 160,000 `repr` texts, 9,548 of 160,000 `%.10f`, 3,580 `%0.6f`, 35,477 `%.18e`,
  62,387 `%.17g` read as other doubles; by kind of value, 186,107 of 200,000 motion-like values (about
  1e-3), 174,222 of 200,000 component-like (about 0.1), 32,236 signal-like (about 3,000), 27,320
  power-like (about 1e7). `%0.6f` and `%.10f` texts below 1e6 mostly read exactly; `%.18e` is 30 %
  wrong in general but exact on DVARS' 6-decimal values. So **most motion, FD, RMSD, signal and
  expansion values in fMRIPrep's confounds files are pandas-perturbed**.
- **larmorx** (decision D3, rule A): by default the correctly rounded value of the same text (only the
  texts pandas accepts as floats are floats: the digits and exponent pandas denotes, then Rust's
  correctly rounded parser); `reproduce` applies pandas' parse at every node boundary of fMRIPrep's
  chain and gives fMRIPrep's bytes. pandas' own `float_precision="round_trip"` is the default's model in
  the check (the same values on every case).
- **On fMRIPrep's real nodes** (the 36 confounds workflows of 21 oracle runs): the default changes
  7,359 of 7,431 cells of the 36 `fdisp` tables and 53,022 of 1,566,383 cells of the 36 `concat` tables
  (the pandas-written columns: motion, FD, RMSD, signals), and nothing in the other table nodes (their
  inputs are nipype's `%.10f`, `%0.6f` and `%.18e` texts, read exactly by both parsers); through the
  expansion, 1,446 of 1,767,020 cells of the expansions and 90,043 of 1,862,699 cells of the final
  tables ([confounds-model.md](confounds-model.md)).

## The two pandas releases differ (steps, no ID; the newest is the default)

- **The exponent**: 3.0.6 reads it with `strtol` (white space after `e`, any digits, a 64-bit `long`;
  above 308 gives ±inf or +0, below -616 gives +0); 2.2.3 with its own loop (at most 17 digits right
  after `e` and its sign, a 32-bit `int`, wrapping; above 308 or a ±inf result is not a float: the column
  becomes text).
- **The integer parsers**: 3.0.6 uses `strtoll`/`strtoull` (an overflowing token followed by any
  character, a space included, is invalid and goes to the float parser; beyond uint64 Python integers);
  2.2.3 parses digit by digit (an overflow as soon as the digits leave the range, whatever follows; the
  column then stays text, missing values NaN). So a column of `%.10f` values from about 1.8e19 on is
  text in pandas 2.2.3 when its first non-integer token is such a value (pandas stops its int64 attempt
  at the first token that fails).
- **The C engine's header renaming** (`TextReader._get_header`, not `io/common.py`'s `dedup_names`): an
  empty name becomes `Unnamed: <i>`; repeats are renamed with the named columns first, then the unnamed
  ones, each the first `<name>.<k>` not already in the header (`[a, a, a.1, '', a, 'Unnamed: 4']` →
  `[a, a.2, a.1, Unnamed: 3, a.3, Unnamed: 4]`). fMRIPrep's `_select_cols` reads the confounds table's
  names that way.

## ujson's 10-decimal formatting (PD-02, rule A)

`to_json` (`TSV2JSON`'s metadata) formats doubles with ujson's `Buffer_AppendDoubleUnchecked` (the same
in both releases): `(value - whole) · 1e10` rounded, truncated, the remainder compared with 0.5 (ties
of an even last digit down, of a last digit 0 up), `%.10g` above `1e16 - 1` and below `1e-15`: 1,642 of
200,028 values get another 10th decimal than the correctly rounded one. larmorx by default writes
`float(f"{x:.10f}")` in the fixed range; `reproduce` ujson's text read by Python's `json`. On fMRIPrep's
metadata (nipype's `%.10f` texts) the two agree: no real node changes.

## Integer columns with missing values (PD-03, rule A)

pandas marks the missing values of an int64 column with int64's minimum (`_maybe_upcast`: `arr ==
na_value`), so a true -9223372036854775808 there becomes NaN. The default keeps it (as a float);
`reproduce` pandas'.

## The table steps (read, verified)

- **fMRIPrep's `FramewiseDisplacement`**: `diff`, the rotations times the radius, `abs().sum(axis=1,
  skipna=False)`. pandas' `DataFrame.sum(axis=1)` transposes (one Fortran-ordered block either way) and
  numpy sums column after column from the first: `((((|dx| + |dy|) + |dz|) + |r·drx|) + |r·dry|) +
  |r·drz|`; `diff` of int64 columns in int64 (wrapping) then float. Its outputs and the motion table are
  gzipped TSVs named `*.tsv.nii.gz` (`fname_presuffix` on the reference's name; pandas infers gzip from
  `.gz`): compared decompressed.
- **`FilterDropped`**, **`RenameACompCor`**: an empty `retained` column (a metadata table with a header
  only) is not a boolean mask for pandas (`is_bool_indexer` of an empty object array is false) but a
  list of column labels: an empty table without columns (`FilterDropped` writes one empty line), and
  `RenameACompCor`'s `all_comp_cor['mask']` then raises `KeyError: 'mask'`; the `KeyError` text for
  other values names `dtype='str'` (3.0.6) or `'object'` (2.2.3). Kept as pandas (rule C: no result to
  correct).
- **PIPE-63** (rule A; fMRIPrep `GatherConfounds`): a shorter table is aligned with the last rows
  (`_adjust_indices`, "missing values at the beginning"), but `pd.concat(axis=1)` joins the row labels
  with `sort=False` (the earlier tables' labels, then the new ones), so when an earlier table is the
  shorter one (no `signals`, a DVARS table of `T - 1` rows first, then a table of `T` rows), the first
  volume's row goes to the end of the file and every other row sits one volume early (`std_dvars` of 3
  rows then FD of 4 → `1.1 0.1 / 1.2 0.2 / 1.3 0.3 / n/a n/a`). fMRIPrep never meets it (its `signals`,
  full length, come first: the 36 real `concat` nodes are identical). Default: the rows in volume order
  (the labels sorted); `reproduce`: pandas' order; the notice when the order changes.
- **PIPE-61** (rule A; niworkflows `AddTSVHeader`): `np.loadtxt` squeezes a one-row table to a vector
  (which `savetxt` writes as one column) and a one-value table to a scalar (`savetxt`'s `ValueError:
  Expected 1D or 2D array, got 0D array instead`): the default keeps the shape (read again with
  `ndmin=2`); `reproduce` niworkflows'.
- **PIPE-62** (rule B; `AddTSVHeader`): a header with another number of names than the table's
  columns is written as given: the default raises `ValueError`, `reproduce` writes the file (notice).
- **`DictMerge`**: plain dictionaries' merge, the same. **`TSV2JSON`**: `index_column`,
  `additional_metadata` added to every row as columns and again to every entry, `drop_columns`,
  `enforce_case` (the keys' case rule), pandas' `to_json` (ujson, PD-02) then Python's `json`.
- **Line ends**: pandas' `to_csv` writes `os.linesep` (`\r\n` on Windows) and `TSV2JSON` opens its JSON
  in text mode (`\r\n` on Windows); larmorx writes `\n` everywhere (Linux's bytes on every platform;
  rule 10's golden files). A finding, no ID.

## Proof (validated)

`check_tables.py`'s full tier in pandas 3.0.6 / fMRIPrep 25.2.7 / niworkflows 1.15.4 (numpy 2.5.4),
pandas 3.0.6 / niworkflows 1.15.3 (numpy 2.3.5) and pandas 2.2.3 / fMRIPrep 25.2.6 / niworkflows 1.15.2
inside fMRIPrep's image, the same counts, CHECK PASS: `parse` 142 columns of 20,000 numbers and 90 odd
tokens, `read` 25 + 24 files, `write` 7 + 7, `ujson` 200,031 texts; the interfaces (`reproduce`
against the originals, the default against the model: the `round_trip` parser, correctly rounded JSON,
rows sorted): 33 FD, 6 `FilterDropped`, 4 `RenameACompCor`, 8 `GatherConfounds`, 7 + 7 `AddTSVHeader`, 3
`DictMerge`, 10 `TSV2JSON` cases; **the 396 real table nodes of the 36 confounds workflows byte for
byte** (`pandas_version="2.2.3"`, `reproduce`). The suite `confounds-fmriprep` (and its smoke case) runs
this script. Speed: [docs/benchmarks/confounds.md](../benchmarks/confounds.md).
