<!-- Record for github.com/bids-standard/pybids. Status: nothing to file: reported as pybids #1270 (2026-08-13, by an fMRIPrep user) and fixed on pybids' master by #1273 (merged 2026-08-24), not in a release yet (0.22.0 of 2026-03-12 is PyPI's newest on 2026-11-10). Optional: ask on #1270 for a release that carries #1273 (the user decides). Register entry: PIPE-37 in docs/findings/upstream-defects.md; larmorx's lx.bids corrects it by default (rule A) exactly as #1273 does (measured: validation/investigations/sdcflows-fieldmap-b/relay32/pipe37_against_pybids_master.py) and reproduces pybids 0.22.0's index with upstream_defects="reproduce". Reproducer: relay32/pipe37_pybids_parent_folders.py. -->
# pybids' index takes entities from the folders above the dataset (fixed on master, not released)

## Status

Already reported and fixed upstream: pybids #1270 ("PyBIDS parsing upstream directory name as
'datatype' (fMRIPrep v25 crash)": a dataset in `/data/projects/motion/BIDS` gave every file
`datatype` `motion`, and fMRIPrep found no BOLD run) was closed by #1273 ("Restrict entity parsing
to the dataset root", merged 2026-08-24): `BIDSLayoutIndexer._index_file` now matches the entity
patterns against the path below the root (`Path('/') / Path(abs_fn.path).relative_to(root)`),
as the ignore patterns already were. No release carries it yet (pybids 0.22.0, 2026-03-12, is the
newest on PyPI on 2026-11-10). Nothing to file; at most a request for a release on #1270.

## What it covers (measured)

The same raw dataset (two participants' T1w) and an fMRIPrep-style derivatives dataset (one
`res-2` preprocessed T1w), stored below four folders and indexed with
`BIDSLayout(root, validate=False, derivatives=...)`:

| Parent folder | pybids 0.22.0 | pybids master (47eb9fb) |
|---|---|---|
| `plain` | subjects `01`, `02`; T1w `datatype` `anat`; the derivative's `res` `2` | the same |
| `sub-pilot` | subjects **`pilot`** only (each file's own label lost) | as `plain` |
| `func` | T1w **`datatype` `func`** | as `plain` |
| `andres-lab` | the derivative's **`res` `lab`**; `get(res='2')` finds nothing | as `plain` |

The raw configuration's patterns need a separator before the key (`[/\\]+sub-`), so only a folder
named exactly like an entity or a datatype matches; the derivatives configuration's `atlas-`,
`roi-`, `label-`, `desc-`, `hemi-`, `seg-`, `res-`, `den-`, `model-` and `subset-` have none, so
any folder name containing one matches (`golden-x` gives `den` `x`).

## What it leaves (for pybids' users to know)

`parse_file_entities` (the function and `BIDSLayout.parse_file_entities`) still parses the path
it is given, which is right for an API. sdcflows' `FieldmapFile` and fMRIPrep's
`extract_entities` call it on absolute paths, so with #1273 the index is right and those two are
not: below `golden-x` both still stop with `ValueError: 'den' is not a recognized entity`, and
below `motion` they now disagree with the index silently (sdcflows' fieldmap-less estimator gets
an empty `IntendedFor`; fMRIPrep's `get_sbrefs` finds no single-band reference). Drafted for
those projects: [sdcflows-entities-from-parent-folders.md](sdcflows-entities-from-parent-folders.md),
[fmriprep-entities-from-parent-folders.md](fmriprep-entities-from-parent-folders.md).
