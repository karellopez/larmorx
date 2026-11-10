<!-- Draft of an issue for github.com/nipreps/sdcflows. Status: draft, not yet filed (no issue found on 2026-11-10 that names this cause; pybids' #1270, fixed on its master by #1273, is the index's side of it, see pybids-entities-from-parent-folders.md). When filed, add the issue's URL here and in README.md. Register entry: SDC-45 in docs/findings/upstream-defects.md; larmorx corrects it by default (rule A: the entities of the path from the participant's sub- folder, a warning when that changed them) and reproduces sdcflows' entities and error with upstream_defects="reproduce". Its fMRIPrep sibling: fmriprep-entities-from-parent-folders.md (PIPE-36). Reproducer: validation/investigations/sdcflows-fieldmap-b/relay32/sdc45_fieldmapfile_entities.py. -->
# `FieldmapFile` takes BIDS entities from the folders above the dataset: the fieldmap-less search stops with `'den' is not a recognized entity`

## Summary

`FieldmapFile.__attrs_post_init__` parses the file's **absolute** path:
`self.entities = parse_file_entities(str(self.path))` (`sdcflows/fieldmaps.py` line 246 in
2.17.1 and on master d0dd398). pybids' `parse_file_entities` uses both its `bids` and
`derivatives` configurations by default, and the derivatives patterns `atlas-`, `roi-`,
`label-`, `desc-`, `hemi-`, `seg-`, `res-`, `den-`, `model-` and `subset-` have no separator
before them (`bids/layout/config/derivatives.json`). So any folder above the dataset whose name
contains one of them gives every file an entity: a dataset stored in `/data/golden-x/ds` gives
`den: 'x'`, one in `/home/andres-lab/...` gives `res: 'lab'`. `find_anatomical_estimators` then
queries the raw layout with those entities (`query = {**base_entities, **target.entities}`,
`utils/wrangler.py` line 630), and pybids refuses the unknown entity:
`ValueError: 'den' is not a recognized entity`. The error is not caught (`find_estimators` calls
`find_anatomical_estimators` outside its `try`), so the run stops.

Who meets it: fieldmap-less estimation (fMRIPrep's `--use-syn-sdc` or `--force syn-sdc`; sdcflows'
BIDS app, where it is on unless `--no-fmapless`) for a participant with a T1w and no field map (or
always with `--force syn-sdc`), on a dataset stored below such a folder. The container wrappers
mount the dataset at `/data`, where nothing above it matches; Apptainer with the host's paths and
bare installations meet it.

The raw configuration's patterns need a separator before them, so they match only folders named
exactly like an entity: below a folder named `motion` (a BIDS datatype), `FieldmapFile` gives the
BOLD run `datatype: 'motion'`. Today pybids' index makes the same mistake for every file (pybids
#1270), so nothing is found at all; with pybids' fix of #1270 (#1273, on its master since
2026-08-24, not yet released) the index is right but `FieldmapFile`'s entities are not, and the
fieldmap-less estimator is built with an empty `IntendedFor`: it corrects nothing, silently.

## Reproducer (no image data needed)

```python
import json
import tempfile
from pathlib import Path

import nibabel as nb
import numpy as np
from bids import BIDSLayout
from sdcflows.fieldmaps import FieldmapFile, clear_registry
from sdcflows.utils.wrangler import find_estimators

base = Path(tempfile.mkdtemp())


def dataset(root):
    for name, shape, meta in (
        ("anat/sub-01_T1w.nii.gz", (4, 4, 4), {}),
        ("func/sub-01_task-rest_bold.nii.gz", (4, 4, 4, 2),
         {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2.0}),
    ):
        path = root / "sub-01" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        nb.Nifti1Image(np.zeros(shape, np.int16), np.eye(4)).to_filename(path)
        path.with_name(path.name.replace(".nii.gz", ".json")).write_text(json.dumps(meta))
    (root / "dataset_description.json").write_text('{"Name": "t", "BIDSVersion": "1.8.0"}')
    return root


for parent in ("plain", "golden-x", "motion"):
    root = dataset(base / parent / "ds")
    bold = root / "sub-01/func/sub-01_task-rest_bold.nii.gz"
    print(parent, "entities:", FieldmapFile(bold).entities)
    clear_registry()
    layout = BIDSLayout(root, validate=False)
    try:
        found = find_estimators(layout=layout, subject="01", fmapless=True, force_fmapless=True)
        print(parent, "estimators:", [e.method.name for e in found],
              "IntendedFor:", [f.metadata.get("IntendedFor") for e in found for f in e.sources])
    except Exception as err:
        print(parent, f"{type(err).__name__}: {err}")
```

Output with sdcflows 2.17.1 and pybids 0.22.0 (CPython 3.12, Linux x86-64; the same with numpy
2.3.5 / SciPy 1.15.3 and numpy 2.5.4 / SciPy 1.18.1):

```
plain entities: {'subject': '01', 'task': 'rest', 'datatype': 'func'}
plain estimators: ['ANAT'] IntendedFor: [['func/sub-01_task-rest_bold.nii.gz'], None]
golden-x entities: {'subject': '01', 'task': 'rest', 'datatype': 'func', 'den': 'x'}
golden-x ValueError: 'den' is not a recognized entity. If you're sure you want to impose this constraint, set invalid_filters='allow'.
motion entities: {'subject': '01', 'task': 'rest', 'datatype': 'motion'}
motion estimators: [] IntendedFor: []
```

With pybids' master (47eb9fb, #1273) the first five lines are the same and the last becomes

```
motion estimators: ['ANAT'] IntendedFor: [[], None]
```

## Expected

The entities of the file within the dataset, so the same estimators whatever folder the dataset is
stored in (here: one fieldmap-less estimator intended for the BOLD run, below every folder).

## Possible fix

Parse the path below the participant's folder, as niworkflows' `DerivativesDataSink` already does
(`niworkflows.utils.bids.relative_to_root`, a dependency sdcflows has):
`self.entities = parse_file_entities(str(relative_to_root(self.path)))`. `relative_to_root` keeps
the path from the nearest `sub-*` folder, and raises for a file below none; keeping today's parse
in that case keeps such inputs working. Where a `BIDSFile` from the layout is given, its own
entities (`get_entities()`) are an alternative once pybids' #1273 is released.
