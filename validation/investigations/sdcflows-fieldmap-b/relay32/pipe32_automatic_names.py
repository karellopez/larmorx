# SPDX-License-Identifier: Apache-2.0
# PIPE-32's companion (docs/upstream-reports/sdcflows-shared-b0fieldidentifier.md): with
# IntendedFor alone (no B0FieldIdentifier) the automatic names of a participant's estimators
# depend on the participants mapped before it in the same process.
import json
import tempfile
from pathlib import Path

import nibabel as nb
import numpy as np
from bids import BIDSLayout
from fmriprep.workflows.base import map_fieldmap_estimation
from sdcflows.fieldmaps import clear_registry

root = Path(tempfile.mkdtemp()) / "ds"


def write(path, shape, meta):
    path.parent.mkdir(parents=True, exist_ok=True)
    nb.Nifti1Image(np.zeros(shape, np.int16), np.eye(4)).to_filename(path)
    path.with_name(path.name.replace(".nii.gz", ".json")).write_text(json.dumps(meta))


for sub in ("01", "02"):
    bold = f"func/sub-{sub}_task-rest_bold.nii.gz"
    for d, pe in (("AP", "j-"), ("PA", "j")):
        write(root / f"sub-{sub}/fmap/sub-{sub}_dir-{d}_epi.nii.gz", (4, 4, 4),
              {"PhaseEncodingDirection": pe, "TotalReadoutTime": 0.05, "IntendedFor": bold})  # fmt: skip
    write(root / f"sub-{sub}" / bold, (4, 4, 4, 2),
          {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2.0})  # fmt: skip
(root / "dataset_description.json").write_text('{"Name": "auto", "BIDSVersion": "1.8.0"}')

layout = BIDSLayout(root, validate=False)
for run in (("01", "02"), ("02",)):
    clear_registry()  # a new fMRIPrep process
    for sub in run:
        bold = [[str(root / f"sub-{sub}/func/sub-{sub}_task-rest_bold.nii.gz")]]
        estimators, _ = map_fieldmap_estimation(layout, sub, None, bold, False, False, False, None)
        print(f"participants {' '.join(run)}: sub-{sub}", [e.bids_id for e in estimators])
