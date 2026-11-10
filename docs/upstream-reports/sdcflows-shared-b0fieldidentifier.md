<!-- Draft of a comment for github.com/nipreps/sdcflows/issues/455 (open since 2024-08-16; fMRIPrep's #3354, closed as not planned, is the same failure). Status: draft, not yet posted. When posted, add the comment's URL here and in README.md. Register entries: PIPE-32 (fMRIPrep's map_fieldmap_estimation) and SDC-36 (sdcflows' BIDS app) in docs/findings/upstream-defects.md; larmorx corrects both by default (rule A: a registry per participant) and reproduces the KeyError with upstream_defects="reproduce". Reproducers: validation/investigations/sdcflows-fieldmap-b/relay32/. -->
# A `B0FieldIdentifier` shared by two participants: a minimal reproducer and a contained fix

*To be posted as a comment on nipreps/sdcflows#455.*

The current BIDS text (as quoted above) asks for a `B0FieldIdentifier` that is unique "within one
participant's tree", so two participants may both call their PEPOLAR pair `pepolar`. Running them
in one process still fails with sdcflows 2.17.1 and fMRIPrep 25.2.7 (and fMRIPrep's master at
21a490fb, 2026-10-07; sdcflows' master at d0dd398, 2026-10-02). On the question whether
multi-echo data explain it: no. The failure needs only an identifier reused **across
participants**, each with its own files; no multi-echo run and no reuse within a participant.

## Why

- `sdcflows.fieldmaps` keeps one module-level registry (`_estimators = EstimatorRegistry()`,
  `fieldmaps.py` line 42), and every `FieldmapEstimation` adds its `bids_id` to it in
  `__attrs_post_init__` (`_estimators[self.bids_id] = self.paths()`, line 446). The registry's
  `__setitem__` refuses a key it holds already: `KeyError: "'pepolar' is already  in mapping"`
  (`utils/bimap.py` line 145; the doubled space comes from `{'a value' * (key in
  self._inverse)}`).
- fMRIPrep builds the participants one after the other in one process
  (`init_fmriprep_wf`: `for subject_id, session_ids in config.execution.processing_groups`,
  `workflows/base.py` line 94 in 25.2.7), and each participant's `map_fieldmap_estimation`
  calls `find_estimators` without clearing the registry. sdcflows' own BIDS app does the same
  (`workflows/fit/base.py` line 44, `cli/main.py` line 68 for `--dry-run`).

## Reproducer (no image data needed)

```python
import json
import tempfile
from pathlib import Path

import nibabel as nb
import numpy as np
from bids import BIDSLayout
from fmriprep.workflows.base import map_fieldmap_estimation

root = Path(tempfile.mkdtemp()) / "ds"


def write(path, shape, meta):
    path.parent.mkdir(parents=True, exist_ok=True)
    nb.Nifti1Image(np.zeros(shape, np.int16), np.eye(4)).to_filename(path)
    path.with_name(path.name.replace(".nii.gz", ".json")).write_text(json.dumps(meta))


for sub in ("01", "02"):
    for d, pe in (("AP", "j-"), ("PA", "j")):
        meta = {"PhaseEncodingDirection": pe, "TotalReadoutTime": 0.05}
        write(root / f"sub-{sub}/fmap/sub-{sub}_dir-{d}_epi.nii.gz", (4, 4, 4),
              {**meta, "B0FieldIdentifier": "pepolar"})
    meta = {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2.0}
    write(root / f"sub-{sub}/func/sub-{sub}_task-rest_bold.nii.gz", (4, 4, 4, 2),
          {**meta, "B0FieldSource": "pepolar"})
(root / "dataset_description.json").write_text('{"Name": "shared", "BIDSVersion": "1.8.0"}')

layout = BIDSLayout(root, validate=False)
for sub in ("01", "02"):  # participant after participant, as init_fmriprep_wf
    bold = [[str(root / f"sub-{sub}/func/sub-{sub}_task-rest_bold.nii.gz")]]
    estimators, mapping = map_fieldmap_estimation(layout, sub, None, bold, False, False, False, None)
    print(f"sub-{sub}:", [e.bids_id for e in estimators])
```

Output (SciPy 1.15.3 and 1.18.1 environments alike; pybids 0.22.0, sdcflows 2.17.1):

```
sub-01: ['pepolar']
Traceback (most recent call last):
  ...
KeyError: "'pepolar' is already  in mapping"
```

Each participant alone maps its own pair (with `for sub in ("02",)`: `sub-02: ['pepolar']`).

## A related effect of the same registry

The automatic names (`auto_00000`, ...) also come from that registry, so a participant's name
depends on the participants built before it in the same run. With `IntendedFor` instead of
`B0FieldIdentifier` (the same dataset otherwise): participants `01 02` give sub-02 `auto_00001`,
sub-02 alone gives `auto_00000`. The name reaches the derivatives (`fmapid-auto00001`, sdcflows'
`workflows/outputs.py` lines 83 and 158), so the same participant's field-map files are named
differently depending on who was processed with it.

## A contained fix

The registry is read only while a participant's estimators are being found: by `find_estimators`
itself (`FieldmapEstimation`, and `wrangler.py` line 457 for the heuristic search) and, in fMRIPrep,
by `get_estimator` through `get_identifier` (`workflows/base.py` lines 1034-1049), both inside
`map_fieldmap_estimation` for the participant being built; nothing reads an earlier participant's
entries (the workflows keep the `FieldmapEstimation` objects themselves). So clearing it per
participant is enough:

- fMRIPrep: `fm.clear_registry()` at the start of `map_fieldmap_estimation` (sdcflows' own test
  fixtures clear it the same way, `conftest.py` lines 89 and 91);
- sdcflows' BIDS app: the same before each participant's `find_estimators` (`workflows/fit/base.py`,
  `cli/main.py`).

This also makes the automatic names a function of the participant alone (`auto_00000` for the
first estimator of every participant), which changes the names of later participants' derivatives
from today's multi-participant runs. If those names must stay, a registry per participant that
starts its counter where the previous one stopped keeps them (that is what our reimplementation
does by default; its compatibility mode reproduces the `KeyError`).
