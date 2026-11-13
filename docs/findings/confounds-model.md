# fMRIPrep's confounds: niworkflows' model expansion and spike regressors

What porting niworkflows 1.15.4's `ExpandModel` (`parse_formula`, `temporal_derivatives`,
`exponential_terms`, the shorthands), `SpikeRegressors` (`spike_regressors`) and
`NormalizeMotionParams` showed, and the defects found (PIPE-43, PIPE-64 to PIPE-70). niworkflows is
Apache-2.0: not restricted. 1.15.4 = 1.15.3 = 1.15.2 on these functions (`diff` of the sources). The
port: `python/larmorx/niworkflows/confounds.py` on `larmorx.confounds._tables`, numpy's float64
`power` in `crates/larmorx-confounds/src/expansion.rs`
([docs/api/niworkflows-helpers.md](../api/niworkflows-helpers.md)). The scripts: `check_model.py`,
`survey_column_order.py` ([validation/investigations/confounds](../../validation/investigations/confounds/README.md)).

## What niworkflows computes (read, verified)

- `ExpandModel` = `pd.read_csv` → `parse_formula(formula, df, unscramble=True)` → `to_csv`. fMRIPrep's
  formula: `(dd1(rps + wm + csf + gsr))^^2 + others`.
- A derivative is numpy's `diff` taken `o` times (exact subtractions; integers in their type, booleans as
  numpy's `not_equal`), the first `o` rows NaN, order 0 the variable itself. A power is pandas' `df ** o`
  block by block: numpy's `ndarray.__pow__` (`fast_scalar_power`): exponent 2 is `square` (any type in
  numpy 2.5.4, gh-29388; booleans give int8), otherwise the `power` ufunc with a float64 scalar (numpy
  2.5.4's stride-0 shortcuts for -1, 0, 0.5, 1, 2; then SVML on AVX-512 CPUs or the C library's `pow`:
  [confounds-numpy.md](confounds-numpy.md)). The order sets (`set(order) - {0}`, `- {1}`) iterate small
  integers ascending (checked: `d7-9`, `^0-2`, `dd6`, `^^6`).
- **Every shorthand is replaced again inside every group** (each nested `parse_formula` calls
  `_expand_shorthand`), so a name holding a shorthand's letters is rewritten once per nesting level, and
  `others` holding a column named like a shorthand (`wm`) is re-expanded inside its group (PIPE-64).
- `SpikeRegressors` = `read_csv` → `spike_regressors` (`np.where(col > thr)`, NaN never flagged; the
  flagged volume always kept, then `for lag in lags: mask = {m + lag for m in mask} | mask`;
  `minimum_contiguous` with `post_final = T + 1`) → `to_csv`. fMRIPrep's settings: FD > 0.5, `std_dvars`
  > 1.5, prefix `motion_outlier`, lags `[0]`, concatenated. **Its output is fMRIPrep's final
  `desc-confounds_timeseries.tsv` byte for byte** (`ds_confounds` copies it: 36 of 36 by SHA-256). A
  table without rows: `max(range(0))`'s `ValueError` for `output='spikes'` (Python 3.12's "max()
  iterable argument is empty"); the mask works.
- `NormalizeMotionParams` = `np.loadtxt` → `np.apply_along_axis(normalize_mc_params, 1, ...)` (FSL,
  AFNI, FSFAST, NIPY; not SPM) → `np.savetxt` (`%.18e`, spaces) as `motion_params.txt`. fMRIPrep 25.2
  has no such node.
- pandas' types: numpy's promotion as pandas' `DataFrame.__array__` does it (float64 for int64 beside
  float64 or uint64; objects for booleans beside other types and for text); `(flag)^2` is int8 in numpy
  2.5.4 (`square`) and int64 in numpy 2.2.6 (`power`): the files' text is the same. pandas 3.0 reads text
  as its `str` type (`to_numpy()`: objects).

## The expansion's column order changes between fMRIPrep processes (PIPE-43; class b, never reproduced)

- **What** (measured, `survey_column_order.py`): within each expanded variable, `parse_formula` orders
  the columns as `list(set(...))` of strings, so by Python's per-process hash seed: in fMRIPrep's 36
  oracle files `x_power2` comes before `x_derivative1_power2` in 169 groups and after it in 155;
  constant within one fMRIPrep process, different between runs. The values do not depend on it.
- **larmorx** (decision D5): one order in both modes, the first-occurrence order (`x`,
  `x_derivative1`, `x_power2`, `x_derivative1_power2`; `others` in the table's order), never the
  hash-seeded one (nondeterminism is never reproduced); a notice (mode "fix", in both modes) when
  another set order would change the result (larmorx parses the formula once more with every set
  reversed and compares). Comparisons are by column name.
- **The check**: niworkflows run with an insertion-ordered `set` injected into its module
  (`check_model.OrderedSet`) gives larmorx's bytes exactly; the unpatched niworkflows by name. Every
  one of the 36 files' orders is one niworkflows can produce (the groups in the table's order, each
  starting with the variable and its first derivative, the two squares either way); in none is it
  larmorx's (0 of 36: with 9 groups the chance is 1/512).

## Defects (rules A to D; `larmorx_core::defects`)

- **PIPE-64** (rule A): the shorthands (`wm`, `gsr`, `rps`, `fd`, `acc`, `tcc`, `dv`, `dvall`, `nss`,
  `spikes`, `others`) are replaced as text, anywhere and again in every group: `std_dvars` becomes
  `std_std_dvarsars` (`dvars` → `std_dvarsars`; `csf_wm` → `csf_white_matter`, so `(others)^2` on
  fMRIPrep's table fails with `KeyError`, which name first following the set order); `dvall` becomes
  `std_dvarsall` (`dv` is replaced first): `dvall` never works; a shorthand or `others` that matches no
  column leaves an empty term (`IndexError: string index out of range`); the names `others` leaves out
  lose every `d<digits>`, `dd<digits>` and `^<digits>` (`head1` → `hea`, so `head1 + others` repeats
  `head1`: PIPE-66). larmorx by default: the shorthands and `others` replaced where they are whole words
  (`[^\s+()]+`), once at the top level; one that matches nothing becomes a term without columns; the
  names `others` leaves out are the formula's words that are neither an exponent nor the text before a
  group. The notice when niworkflows' text would differ (at any level).
- **PIPE-65** (rule B for ignored text, A for the rest): `parse_expression` reads the operators with
  regular expressions over the whole term: `foo(a)`, `(a)b`, `d1x(a)`, `d1(a)(b)`, `dd1-2(a)` (`dd1`
  taken), `(a)^ ^2` give `a`'s columns (the rest ignored); `((a)^2)^3` finds both exponents → `TypeError:
  _order_as_range() takes 1 positional argument but 2 were given`; `d1a`, `a^2` (no group) → `TypeError:
  'NoneType' object is not iterable`. larmorx by default: a term is a name, or `[derivative] (group)
  [exponent]` with the derivative `dd<n>|d<n>[-[<m>]]` and the exponent `^^<n>|^<n>[-[<m>]]` (the
  original's own patterns, whole), else `ValueError`; the exponent read after the group only
  (`((x)^2)^3` = `x_power2_power3`); a term without a complete group is a name. Both refuse, as
  niworkflows: an empty written term (`a +`, `a + + b`, `()`: `IndexError`), `d1-(a)` (`ValueError:
  invalid literal for int()`), `d3-1(a)` (`TypeError: reduce() of empty iterable`), unbalanced
  parentheses (`KeyError` of the text), a name that is not a column (`KeyError`), a parent column whose
  name without `_power<n>`/`_derivative<n>` is not a column when unscrambling (`KeyError`).
- **PIPE-66** (rule A): a variable reached by two terms is repeated: `a + (a)^^2` gives `a, a, a_power2`
  in `parse_formula` and `a` four times with `a_power2` in `ExpandModel`'s file (pandas' `df[list]`
  selects every column of a repeated name each time it is named: n² copies); a group that repeats one
  (`dd1(a + (a))`) fails with pandas' `ValueError: Shape of passed values is (6, 4), indices imply (6,
  2)` (the same in pandas 2.2.3 and 3.0.6). larmorx: each name once (its first column) after every
  `concat`.
- **PIPE-67** (rule A): the lags compound: `[1, 2]` and `[0, 1, 2]` flag the third volume after a spike
  too (`[-1, -2]`: the third before). larmorx: the union of the flagged volumes and each lag added to
  them. The flagged volume itself is always kept (`| mask`), also for `lags=[1]`: kept (rule C:
  plausibly deliberate, the docstring's example uses `[0, 1]`).
- **PIPE-68** (rule A): `minimum_contiguous`'s `post_final = T + 1` counts the last unflagged run one
  volume longer (`T = 10`, a spike at 7, minimum 3: volumes 8 and 9 kept; minimum 4 flags them; no spike,
  minimum 11: nothing flagged). larmorx: `post_final = T`.
- **PIPE-69** (rule B): a criterion other than `'<'` and `'>'` is skipped; when every one is, `TypeError:
  reduce() of empty iterable with no initial value`. larmorx: `ValueError` (the function only: the
  interface always passes `'>'`).
- **PIPE-70** (rule A): `NormalizeMotionParams` on a one-row file: `np.loadtxt` gives a vector →
  `AxisError: axis 1 is out of bounds for array of dimension 1`. larmorx keeps the row (`ndmin=2` when the
  file holds one row of more than one value). One value, one column, an empty file: numpy's `AxisError`
  in both modes (no sensible result).
- **Powers, GLIBC-07 and NP-04** (no new ID): glibc's `pow` (numpy's on CPUs without AVX-512) is not
  correctly rounded on 611 of 140,011 cubes (0.44 %; 141 of the 4th powers, about 100 of the 5th to the
  9th); numpy's SVML `pow` (AVX-512, the wheels' default on the main machine) differs from glibc's on
  3,669 (2.6 %) of the cubes and 2,730 to 3,376 of the other exponents: NP-04's class (the CPU decides),
  not followed. The default is the correctly rounded `pow`, `reproduce` glibc's. fMRIPrep's formula only
  squares (`x · x`, exact): none of this reaches its files.

## On fMRIPrep's real nodes (validated)

`check_model.py`'s `real` section, the 36 workflows of 21 oracle runs: `model_expansion` by name and
`spike_regressors` byte for byte with `pandas_version="2.2.3"` and `reproduce`; the chain larmorx →
larmorx equals the derivative by name. The default changes 1,446 of 1,767,020 cells of the 36
expansions and 90,043 of 1,862,699 cells of the final tables (PD-01: pandas reads the expansion's
17-digit texts as other doubles, [confounds-pandas.md](confounds-pandas.md)). The full tier of every
section, CHECK PASS in numpy 2.5.4 / pandas 3.0.6, numpy 2.3.5 / pandas 3.0.6 and numpy 2.2.6 / pandas
2.2.3 inside fMRIPrep's image, the same counts, with numpy's AVX-512 kernels off as every run of the
record; then `power` with them on (SVML's differences counted apart, NP-04). The suite
`confounds-fmriprep` runs it (its full tier also with AVX-512 on). Speed:
[docs/benchmarks/confounds.md](../benchmarks/confounds.md).
