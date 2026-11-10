# SPDX-License-Identifier: Apache-2.0
# PIPE-32 (docs/upstream-reports/sdcflows-shared-b0fieldidentifier.md): two participants whose
# PEPOLAR pairs share a B0FieldIdentifier, mapped one after the other as init_fmriprep_wf does
# (the participants to map: the arguments, both by default).
import json
import sys
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
              {**meta, "B0FieldIdentifier": "pepolar"})  # fmt: skip
    meta = {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2.0}
    write(root / f"sub-{sub}/func/sub-{sub}_task-rest_bold.nii.gz", (4, 4, 4, 2),
          {**meta, "B0FieldSource": "pepolar"})  # fmt: skip
(root / "dataset_description.json").write_text('{"Name": "shared", "BIDSVersion": "1.8.0"}')

layout = BIDSLayout(root, validate=False)
for sub in sys.argv[1:] or ("01", "02"):  # participant after participant, as init_fmriprep_wf
    bold = [[str(root / f"sub-{sub}/func/sub-{sub}_task-rest_bold.nii.gz")]]
    estimators, mapping = map_fieldmap_estimation(
        layout, sub, None, bold, False, False, False, None
    )
    print(f"sub-{sub}:", [e.bids_id for e in estimators])
