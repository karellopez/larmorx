# nipype's interface runner: `larmorx nipype run`

`larmorx nipype run` (and `lx.nipype.run`) is nipype 1.12.0's `nipypecli run` for the
interfaces larmorx has ported: it lists a module's interfaces, prints an interface's help, and
runs one interface from the shell, in the current folder, without nipype, click or traits. The
same arguments, listing, help, printed inputs and outputs, files, messages and exit statuses as
nipype's own command line (nipype 1.12.0 with niworkflows 1.15.4 and fMRIPrep 25.2.7, the newest
releases), checked case by case against it. It serves the modules of fMRIPrep's confound
estimation:

| Module | Interfaces larmorx runs | Listed, not run |
|---|---|---|
| `nipype.algorithms.confounds` | `ACompCor`, `CompCor`, `TCompCor`, `ComputeDVARS`, `FramewiseDisplacement`, `NonSteadyStateDetector`, `TSNR` | `BaseInterface`, `SimpleInterface` (nipype's base classes) |
| `niworkflows.interfaces.confounds` | `ExpandModel`, `SpikeRegressors`, `NormalizeMotionParams`, `FSLMotionParams`, `FSLRMSDeviation`, `FramewiseDisplacement` | `SimpleInterface` |
| `niworkflows.interfaces.images` | `SignalExtraction`, `RegridToZooms` | `Conform`, `TemplateDimensions` (not ported), `IntraModalMerge`, `RobustAverage` (ported, below), `SimpleInterface` |
| `niworkflows.interfaces.morphology` | `BinaryDilation`, `BinarySubtraction` | `SimpleInterface` |
| `niworkflows.interfaces.nibabel` | `ApplyMask`, `Binarize`, `IntensityClip`, `MergeSeries`, `ReorientImage`, `RegridToZooms`, `GenerateSamplingReference` | `BinaryDilation` (nibabel's, deprecated by niworkflows), `DemeanImage`, `FilledImageLike`, `MapLabels`, `MergeROIs`, `SplitSeries`, `SimpleInterface` |
| `niworkflows.interfaces.utility` | `AddTSVHeader`, `DictMerge`, `TSV2JSON` | `JoinTSVColumns`, `KeySelect` (nipype cannot instantiate it without fields), `BaseInterface`, `SimpleInterface` |
| `niworkflows.interfaces.bold` | `NonsteadyStatesDetector` | `SimpleInterface` |
| `niworkflows.interfaces.patches` | `RobustACompCor`, `RobustTCompCor` | `FreeSurferSource` |
| `fmriprep.interfaces.confounds` | `aCompCorMasks`, `FSLMotionParams`, `FSLRMSDeviation`, `FramewiseDisplacement`, `FilterDropped`, `RenameACompCor`, `GatherConfounds` | `FMRISummary` (a report: with larmorprepx's reports), `SimpleInterface` |

37 interfaces of the 61 these modules hold. Each runs through the larmorx function that ports it
(the confounds' [docs/api/confounds.md](confounds.md), niworkflows' helpers
[docs/api/niworkflows-helpers.md](niworkflows-helpers.md)), with the newest libraries' arithmetic
(numpy 2.5.4, SciPy 1.18.1, pandas 3.0.6). An interface that is listed but not run prints its
help as nipype does; running it is an error (`error: larmorx has not ported ...`, exit status 1).
`IntraModalMerge` and `RobustAverage` are ported (`lx.niworkflows.intra_modal_merge`,
`robust_average`) but not run here yet: they run FSL's `mcflirt` and AFNI's `3dvolreg` as
programs whose messages nipype relays through its log, line by line, which larmorx's ports in this
process do not print (a follow-up).

## The command line

```
larmorx [--n-threads N] [--upstream-defects fix|reproduce] nipype run [--list] <module> [<Interface> [inputs]]
```

| nipype's `nipypecli run` | `larmorx nipype run` | Notes |
|---|---|---|
| `nipypecli run <module> --list`, `nipypecli run <module>` | the same | `Available Interfaces:` and the names of the interface classes in the module's namespace, sorted (those it imports included), four blanks before each |
| `nipypecli run <module> <Interface>` | the same | the interface's help (argparse's, with nipype's descriptions of each input: their texts are nipype's, niworkflows' and fMRIPrep's), then its usage; the width is argparse's (`COLUMNS`, else the terminal's) |
| `nipypecli run <module> <Interface> <inputs>` | the same | the mandatory inputs positional, in name order (lists with one or more values), the others `--name value` (lists with zero or more), booleans `--name` (they set true); `setting function inputs`, every input as nipype prints it (`name = value`, `<undefined>`), the interface run in the current folder (its files written there), every output as nipype prints it |
| `-h`, `--help` | the same | a flag the command never reads (click's): `run <module> <Interface> -h` prints the help because the flag leaves no input; `--` passes `-h` to argparse |
| `nipypecli` `crash`, `search`, `show`, `version`, `convert` | not ported | `error: unknown nipype tool` (exit status 2) |
| (the environment) | `--n-threads N` (`OMP_NUM_THREADS`), `--upstream-defects fix|reproduce` (`LARMORX_UPSTREAM_DEFECTS`), before the family | the threads of larmorx's computations (the results do not depend on them; default every logical CPU); D13's mode, which also chooses how the command line itself behaves (below) |

Exit statuses: 0; 2 for a usage error (click's, as `Usage: larmorx nipype run [OPTIONS] [MODULE]
[INTERFACE]` and `Error: Invalid value for '[MODULE]': got None.`, or argparse's); 1 for an
error, which nipype ends with a Python traceback: larmorx prints the traceback's first and last
lines (`Traceback (most recent call last):` and the exception with nipype's message: traits'
`TraitError` of a value an input refuses, nipype's `ValueError` of a missing input it requires,
`OSError` of two exclusive inputs, `AttributeError` of an unknown interface, the interface's own
errors). nipype's log lines (its interfaces' warnings, e.g. CompCor's default of 6 components)
are printed as nipype prints them, on standard output with nipype's time stamp
(`261113-07:09:13,278 nipype.interface WARNING:` and the message on the next line). The standalone
`larmorx` program lists the tool and says that it is a Python program of the Python package
(exit status 127).

Not as nipype's (documented, not defects): a module larmorx does not serve is an error of its own
(`error: larmorx does not run the interfaces of '<module>' ...`, exit status 1), where nipype
imports any module (or ends with `ImportError` when there is none); a name of the module's
namespace that is not an interface is `AttributeError`, as an unknown name; nipype's version check
over the network (etelemetry) is not made; the plots of `ComputeDVARS` and
`FramewiseDisplacement` (`--save_plot`) are not drawn yet (`NotImplementedError`: they come with
larmorprepx's reports). Python's own suggestions on the traceback's last line (`Did you mean: ...`)
are Python's, not the message's.

## The defects of nipype's command line

Corrected by default; `--upstream-defects reproduce` (or `upstream_defects="reproduce"`) gives
nipype's behaviour, its errors included. A notice (`UpstreamDefectWarning`) says when a
correction changed what an input received.

| ID | nipype's command line does | larmorx by default |
|---|---|---|
| PIPE-81 | gives an input of a compound trait (`traits.Either`) the argparse type of the input before it in name order (a variable left from the loop's previous iteration): CompCor's `num_components` reads text, so a number of components is refused (only `all` works); `save_pre_filter` and `save_metadata` read a float, so a file name or `True` is refused; and cannot build the parser of an interface whose first input is compound (`UnboundLocalError`: niworkflows' `TSV2JSON`) or that has a list of compound items (`AttributeError`: niworkflows' `DictMerge`, fMRIPrep's `FMRISummary`): those interfaces cannot be run, nor their help printed | builds every parser and reads a compound input's text by its alternatives, the first that takes it (`3` an integer, `all` the text; `True` the boolean, `pre.tsv` a file) |
| PIPE-82 | reads an input's text by the type of its default value, not by its trait: a float input whose default is an integer reads only integers (CompCor's `high_pass_cutoff` 128, `FramewiseDisplacement`'s `radius` 50: `--radius 40.5` is refused); the texts of a tuple (`aCompCorMasks`' mandatory `bold_zooms`, `RegridToZooms`' mandatory `zooms`, the plots' `figsize`), of an optional dictionary and of an enumeration's values that are not text (CompCor's `pre_filter False`, `GenerateSamplingReference`'s `xform_code 4`) reach the trait unconverted and are refused whatever is given; a list of booleans reads every text but the empty one as true (`bool("False")`) | reads each text as its trait reads it: a float input as a float; a tuple as a Python literal (`2,2,2.5` or `(2, 2, 2.5)`); a dictionary as JSON (as nipype reads a mandatory one); an enumeration's value by its text (`False`, `4`); a boolean from `True`/`1` or `False`/`0`; a list given as one text (an optional `traits.Either` of a list) as a Python literal (`["mask"]`) |

Kept (PIPE-83, rule C: nipype's design, documented): a boolean input whose default is true is a
flag that only sets true, so it cannot be turned off from the command line (`ComputeDVARS`'
`remove_zerovariance`, `save_std`, `IntraModalMerge`'s `hmc`); two mandatory list inputs on
either side of another positional input are split by argparse's greedy matching
(niworkflows' `SignalExtraction` with several labels: `class_labels`, `in_file`, `label_files`),
so fMRIPrep's own call of five labels ends in the interface's `Number of masks must match number
of labels`, as with nipype's command line.

The interfaces' own defects are those of their functions (the confounds' page): their corrections
apply here too, with their notices, and `reproduce` gives nipype's files.

## Python

| larmorx | nipype | Notes |
|---|---|---|
| `lx.nipype.run(argv=None, *, prog="larmorx nipype run", upstream_defects=None, n_threads=None, env=None)` → exit status | `nipype.scripts.cli:cli` (`nipypecli run`) | `argv`: the arguments after `run` (default `sys.argv[1:]`); `prog` the command's name in the messages; `upstream_defects` defaults to `LARMORX_UPSTREAM_DEFECTS` (else `"fix"`), `n_threads` to `OMP_NUM_THREADS` (else every logical CPU); `env` the environment read (default `os.environ`); writes to `sys.stdout` and `sys.stderr`, the interface's files to the current folder |
| `lx.nipype.list_interfaces(module)` → names | `nipype.scripts.instance.list_interfaces` | the names `--list` prints; `ValueError` for a module not served |
| `lx.nipype.ported_interfaces()` → `{module: names}` | | the interfaces `run` runs |
| `lx.nipype.MODULES` | | the nine modules |

## How it is built and checked

The parsers are nipype's own: `python/larmorx/nipype/interfaces.json` holds, for every interface
of the nine modules, the `add_argument` calls nipype's `add_args_options` makes (recorded from
nipype itself, its help texts included), the corrected parser's calls (PIPE-81, PIPE-82), each
input's default as nipype prints it, its validation (the trait's kind, bounds, values and traits'
text for its error message), its `xor`, `requires` and `deprecated` metadata, and the outputs;
`validation/investigations/confounds/nipype_cli_spec.py` writes it from nipype 1.12.0,
niworkflows 1.15.4, fMRIPrep 25.2.7 and traits 7.2.0 (`--check` compares). larmorx builds the
parser with Python's argparse from those calls (so the help is argparse's, as nipype's), emulates
click's parsing of `run`, traits' validation and messages, nipype's `run_instance`,
`_check_mandatory_inputs` and printing, and calls the ported function
(`python/larmorx/nipype/_run.py`, `_adapters.py`).

**Status: `experimental`.** Checked against nipype's own `nipypecli run`
(`validation/investigations/confounds/check_nipype_cli.py`, the newest environment; finding
[confounds-nipype-cli.md](../findings/confounds-nipype-cli.md)): the exit status, standard
output, standard error and every file written, byte for byte, with `reproduce`, and with the
default (identical but where a correction applies: then against nipype's interface run through its
Python API on the same inputs); the click layer, all 61 helps at two widths, argparse's and traits'
errors, every interface larmorx runs with its options, and fMRIPrep's real calls of the confounds
nodes of three reference runs (single-band, multiband with 280 volumes, multi-echo): 226 of 226
cases, 51 real calls among them (15 of which nipype's command line refuses: PIPE-81, PIPE-82); the
parity suite `confounds-nipype` runs the check as its cases `cli-quick-e1` and `cli-full-e1`
(the record [confounds-nipype](../validation/confounds-nipype.md), standard tier, 2026-11-13: 25 of 25
cases, `cli-quick-e1` among them, 18 of 18 checks). The
golden case `cli.nipype.run`; `tests/python/test_nipype_run.py`.
