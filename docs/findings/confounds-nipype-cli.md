# nipype's command line, `nipypecli run`: what it does with the confounds' interfaces

What porting nipype 1.12.0's `nipypecli run` (`nipype/scripts/cli.py`, `scripts/utils.py`,
`scripts/instance.py`, `utils/nipype_cmd.py`) as `larmorx nipype run` showed, for the nine
modules of fMRIPrep's confound estimation (nipype's `algorithms/confounds.py`, niworkflows
1.15.4's `interfaces/{confounds, images, morphology, nibabel, utility, bold, patches}.py`,
fMRIPrep 25.2.7's `interfaces/confounds.py`): its defects (PIPE-81, PIPE-82, corrected by default;
PIPE-83, kept) and how fMRIPrep's own calls fare through it. nipype, niworkflows and fMRIPrep are
Apache-2.0, traits BSD-3-Clause: not restricted. The port: `python/larmorx/nipype/`
([docs/api/nipype-run.md](../api/nipype-run.md)). The scripts: `nipype_cli_spec.py` (the
interfaces' parsers and traits, read from nipype itself), `check_nipype_cli.py` with
`check_nipype_cli_cases.py` (larmorx against nipype's command line, case by case; the parity
suite `confounds-nipype` runs it as its cases `cli-quick-e1` and `cli-full-e1`)
([validation/investigations/confounds](../../validation/investigations/confounds/README.md)).

## How nipype's command line reads an interface

`nipypecli run <module> <Interface> [inputs]` is a click command with unknown options passed
through (`ignore_unknown_options`, `allow_extra_args`); its `--list` lists the module's
interface classes (every `Interface` subclass in its namespace, imported ones included: nipype's
`BaseInterface` and `SimpleInterface` appear in eight of the nine modules), and its `-h`/`--help`
is a flag of its own that the command never reads (so click's own help option is gone):
`run -h` ends in click's `Invalid value for '[MODULE]': got None.` (exit status 2), and `run
<module> <Interface> -h` prints the interface's help only because the flag leaves no input for
argparse. click's parser's own errors (`--list=x`: `Option '--list' does not take a value.`) are
printed without the usage line, a parameter's error with it.

The interface's parser is built by `add_args_options` from its input traits, in name order: a
mandatory input positional (`nargs='+'` for a list), the others `--name` (`nargs='*'` for a list),
a boolean `store_true` with the trait's default; the argparse type is `type(trait_type.
default_value)` when it is `str`, `int` or `float` (and a list's item type, with `bool`), a
mandatory dictionary `json.loads`. The values are then set on the inputs one by one, in name order
(`run_instance`), and traits validates each: `TraitError` is not caught (only `ValueError` is,
which no trait raises), so a refused value ends the program with a traceback. The 61 interfaces'
parsers, the inputs' validations and traits' texts, recorded by `nipype_cli_spec.py`
(`python/larmorx/nipype/interfaces.json`), are what larmorx builds its parsers and messages from.

## PIPE-81: compound inputs get the previous input's type (corrected; `reproduce` keeps it)

`add_args_options` sets `trait_type` only for an input that is not compound
(`if not spec.is_trait_type(traits.TraitCompound): trait_type = type(...)`): for a
`traits.Either` input the variable keeps the value of the input before it in name order, so the
compound input reads its text with that input's type. On the nine modules:

| Interface | Input (its trait) | Type it gets | So |
|---|---|---|---|
| nipype's `CompCor`, `ACompCor`, `TCompCor`, niworkflows' `RobustACompCor`, `RobustTCompCor` | `num_components` (`Either(Range(low=1), Enum('all'))`) | `str` (from `merge_method`) | a number of components is refused (`'2' <class 'str'>`): only `all` works |
| the same five | `save_pre_filter`, `save_metadata` (`Either(Bool, File)`) | `float` (from `repetition_time` and `regress_poly_degree`'s neighbours) | a file name is argparse's `invalid float value`, and so is `True`; a number is refused by the trait |
| niworkflows' `SpikeRegressors` | `minimum_contiguous` (`Either(None, Int)`) | `int` (from `lags`) | integers work (by chance); `None` cannot be given |
| niworkflows' `RegridToZooms` (both modules) | `smooth` (`Either(Bool, Float)`) | `int` (from `order`) | `True` and `1.5` are refused |

When the first input in name order is compound the variable does not exist yet: niworkflows'
`TSV2JSON` (`additional_metadata`) ends in `UnboundLocalError: cannot access local variable
'trait_type' where it is not associated with a value`; a list of compound items reads
`spec.inner_traits[0].trait_type.default_value`, which a `TraitCompound` has not: niworkflows'
`DictMerge` and fMRIPrep's `FMRISummary` end in `AttributeError: 'TraitCompound' object has no
attribute 'default_value'`. Their parsers cannot be built, so these three interfaces cannot be run
nor their help printed. Rule A (the input makes sense, the result is wrong): larmorx builds every
parser with no type for a compound input (`nipype_cli_spec.add_args_options_corrected`) and reads
its text by the trait's alternatives, the first that takes it.

## PIPE-82: texts not read as their traits read them (corrected; `reproduce` keeps it)

The argparse type is the default value's Python type, not the trait's: a `traits.Float` whose
default is an integer reads only integers (CompCor's `high_pass_cutoff` 128 in the five CompCor
interfaces, `FramewiseDisplacement`'s `radius` 50 in nipype's, niworkflows' and fMRIPrep's,
niworkflows' nibabel `BinaryDilation`'s `radius`): `--radius 40.5` is argparse's `invalid int
value`, and so is `--high_pass_cutoff 128.0`, fMRIPrep's own value. Inputs whose default is
neither text nor a number get no type, and their text reaches the trait unconverted, which refuses
it whatever is given: every tuple (fMRIPrep's `aCompCorMasks`' mandatory `bold_zooms`,
niworkflows' `RegridToZooms`' mandatory `zooms`, `Conform`'s `target_shape` and `target_zooms`,
the plots' `figsize`), an optional dictionary (niworkflows' `MapLabels`' `mappings`; a mandatory
one is read by `json.loads`), an enumeration's values that are not text (CompCor's `pre_filter
False`, `RobustAverage`'s `mc_method None`, `GenerateSamplingReference`'s `xform_code` 2 or 4).
And a list of booleans reads its items with `type=bool`, so every text but the empty one is true
(`RobustAverage`'s `t_mask`: `False 0` is `[True, True]`). `aCompCorMasks` and `RegridToZooms`
cannot be run at all. Rule A: larmorx reads each text by its trait: a float input as a float; a
tuple as a Python literal (`2,2,2.5` or `(2, 2, 2.5)`); a dictionary as JSON; an enumeration's
value by its text; a boolean from `True`/`1` or `False`/`0`; a list given as one text (an optional
`Either` of a list: `TSV2JSON`'s `drop_columns`) as a Python literal. Where the corrected parser's
type differs from nipype's, the input is parsed as text and read afterwards, so a text argparse
would refuse is still argparse's error (`invalid float value`: the corrected type's name).

## PIPE-83: the command line's limits (kept, rule C)

Two behaviours are nipype's design and kept: a boolean input whose default is true is a
`store_true` flag, which only sets true, so it cannot be turned off (`ComputeDVARS`'
`remove_zerovariance` and `save_std`, `IntraModalMerge`'s `hmc`, niworkflows' `nonnegative`);
and two variable-length positional inputs are split by argparse's greedy matching: niworkflows'
`SignalExtraction` (`class_labels`, `in_file`, `label_files`) given fMRIPrep's five labels and
five files takes nine values as labels, so the interface ends in `ValueError: Number of masks must
match number of labels`, through nipype's command line and larmorx's alike. No switch: the same
in both modes.

## fMRIPrep's own calls through the command line

`check_nipype_cli.py --tier full` builds the command line of every confounds node of the first
BOLD run of three oracle runs (ds000005 single-band, ds005454 multiband with 280 volumes,
ds003592 multi-echo; the node's own inputs from its `_report/report.rst`, fMRIPrep's container
paths mapped to the run): 51 calls of the 17 node kinds a command line can give, every one larmorx
runs. 15 of them nipype's command line refuses (PIPE-81 or PIPE-82: the 9 CompCor nodes,
as fMRIPrep sets `save_pre_filter` and `save_metadata` to `True` and its `high_pass_cutoff` is
`128.0`; the 3 `fdisp`, their `radius` `50.0`; the 3 `acc_masks`, their `bold_zooms`); larmorx runs
those by default, its printed inputs and its files those of nipype's interface run through its
Python API on the same inputs (but where the interfaces' own corrections apply: then the printed
inputs). With `reproduce` every one of the 51 calls gives nipype's command line's exit status,
output, error and files (log `oracles/logs/confounds-r2i-check-real.log`: 51 of 51; the whole full tier,
226 of 226 cases: `oracles/logs/confounds-r2i-check-nipype-cli.log`).

## What else it showed

- nipype's log lines (its interfaces' `IFLOGGER` messages) go to standard output with a time
  stamp (`%y%m%d-%H:%M:%S,<milliseconds>`), at INFO and above; larmorx prints its ports'
  messages so.
- traits' error messages: `The '<name>' trait of a(n) <InputSpec> instance must be <info>, but a
  value of <repr> <type> was specified.`, `Each element of the ...` for a list's item, a path's
  value shown as its text (nipype's `BasePath` passes `str(value)`), the article by the spec's
  first letter (`a _aCompCorMasksInputSpec`). nipype's `OutputMultiObject` prints a list of one
  item as the item (TCompCor's `high_variance_masks`).
- `CompCor`'s `pre_filter_file` and `metadata_file`, niworkflows' `ExpandModel`'s and
  `SpikeRegressors`' `output_file` are the names given, relative ones kept relative.
- `IntraModalMerge` and `RobustAverage` run FSL's `mcflirt` and AFNI's `3dvolreg` as programs
  whose standard error nipype relays through its log line by line (with the programs' own time
  stamps); `RobustAverage`'s average and affines through larmorx's ports are nipype's, but these
  two interfaces are not run by `larmorx nipype run` yet (a follow-up: the relayed messages, and
  `3dvolreg`'s `-1Dfile` and `-maxdisp1D` files, which differed in one trial).
