<!-- Draft of an issue for github.com/nipreps/fmriprep. Status: draft, not yet filed (no issue found on 2026-11-10 that names this cause; pybids' #1270, reported by an fMRIPrep user and fixed on pybids' master by #1273, is the index's side of it, see pybids-entities-from-parent-folders.md). When filed, add the issue's URL here and in README.md. Register entry: PIPE-36 in docs/findings/upstream-defects.md; larmorx's BOLD stages are not ported yet: rule A when they are (the entities of the BIDS part, upstream_defects="reproduce" fMRIPrep's). Its sdcflows sibling: sdcflows-entities-from-parent-folders.md (SDC-45). Reproducer: validation/investigations/sdcflows-fieldmap-b/relay32/pipe36_extract_entities.py. -->
# `extract_entities` parses absolute BOLD paths: below a folder such as `golden-x` every BOLD run stops with `'den' is not a recognized entity`

## Summary

`fmriprep.utils.bids.extract_entities` parses each path it is given with pybids'
`parse_file_entities(f)` (`utils/bids.py` line 350 in 25.2.7, line 515 on master 21a490fb). Its
docstring's examples are relative paths, but its callers pass the layout's absolute ones:
`get_sbrefs` (`workflows/bold/fit.py` line 84), which `init_bold_fit_wf` calls for every BOLD
series (line 212), and `init_single_subject_wf` for `collect_derivatives` with `--derivatives`
(`workflows/base.py` line 864). `parse_file_entities` uses pybids' `bids` and `derivatives`
configurations, and the derivatives patterns `atlas-`, `roi-`, `label-`, `desc-`, `hemi-`, `seg-`,
`res-`, `den-`, `model-` and `subset-` have no separator before them, so a folder above the
dataset whose name contains one gives every BOLD run that entity (`/data/golden-x/ds` gives
`den: 'x'`, `/home/andres-lab/...` gives `res: 'lab'`). `get_sbrefs` then queries the raw layout
with it and pybids refuses: `ValueError: 'den' is not a recognized entity`, with or without
field maps.

A folder named exactly like a datatype (`motion`, `func`, ...) gives `datatype` instead. Today
pybids' index makes the same mistake for every file (pybids #1270: "No BOLD images found");
with pybids' fix of #1270 (#1273, on its master since 2026-08-24, not yet released) the index is
right but `extract_entities` still says `datatype: 'motion'`, and `get_sbrefs` silently finds no
single-band reference.

The container wrappers mount the dataset at `/data`, where nothing above it matches; Apptainer
with the host's paths and bare installations meet it.

## Reproducer (no image data needed)

```python
import json
import tempfile
from pathlib import Path

import nibabel as nb
import numpy as np
from bids import BIDSLayout
from fmriprep.utils.bids import extract_entities
from fmriprep.workflows.bold.fit import get_sbrefs

base = Path(tempfile.mkdtemp())
for parent in ("plain", "golden-x", "motion"):
    root = base / parent / "ds"
    for name, shape in (("sbref", (4, 4, 4)), ("bold", (4, 4, 4, 2))):
        path = root / f"sub-01/func/sub-01_task-rest_{name}.nii.gz"
        path.parent.mkdir(parents=True, exist_ok=True)
        nb.Nifti1Image(np.zeros(shape, np.int16), np.eye(4)).to_filename(path)
        path.with_name(path.name.replace(".nii.gz", ".json")).write_text(
            json.dumps({"RepetitionTime": 2.0, "EchoTime": 0.03})
        )
    (root / "dataset_description.json").write_text('{"Name": "t", "BIDSVersion": "1.8.0"}')
    layout = BIDSLayout(root, validate=False)
    bold = str(root / "sub-01/func/sub-01_task-rest_bold.nii.gz")
    print(parent, "entities:", extract_entities([bold]))
    try:
        print(parent, "sbrefs:", [Path(f).name for f in get_sbrefs([bold], {}, layout)])
    except Exception as err:
        print(parent, f"{type(err).__name__}: {err}")
```

Output with fMRIPrep's master (21a490fb; 25.2.7's `extract_entities` is the same code and its
`get_sbrefs` makes the same query) and pybids 0.22.0 (CPython 3.12, Linux x86-64):

```
plain entities: {'subject': '01', 'task': 'rest', 'suffix': 'bold', 'datatype': 'func', 'extension': '.nii.gz'}
plain sbrefs: ['sub-01_task-rest_sbref.nii.gz']
golden-x entities: {'subject': '01', 'task': 'rest', 'suffix': 'bold', 'datatype': 'func', 'extension': '.nii.gz', 'den': 'x'}
golden-x ValueError: 'den' is not a recognized entity. If you're sure you want to impose this constraint, set invalid_filters='allow'.
motion entities: {'subject': '01', 'task': 'rest', 'suffix': 'bold', 'datatype': 'motion', 'extension': '.nii.gz'}
motion sbrefs: ['sub-01_task-rest_sbref.nii.gz']
```

With pybids' master (47eb9fb, #1273) the first five lines are the same and the last becomes

```
motion sbrefs: []
```

## Expected

The BOLD run's entities within the dataset, whatever folder it is stored in: the single-band
reference found below every folder.

## Possible fix

Parse the path below the dataset in `extract_entities`, as niworkflows' `DerivativesDataSink`
does: `parse_file_entities(str(relative_to_root(f)))` (`niworkflows.utils.bids.relative_to_root`,
the path from the nearest `sub-*` folder), or the path relative to `layout.root` where the caller
has the layout. sdcflows' `FieldmapFile` has the same parse (reported there; it stops
`--use-syn-sdc` below the same folders).
