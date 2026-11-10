# SPDX-License-Identifier: Apache-2.0
# PIPE-37 (docs/upstream-reports/pybids-entities-from-parent-folders.md): pybids' index of the
# same raw dataset and the same derivatives below four parent folders.
import json
import tempfile
from pathlib import Path

import nibabel as nb
import numpy as np
from bids import BIDSLayout

base = Path(tempfile.mkdtemp())


def nifti(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    nb.Nifti1Image(np.zeros((4, 4, 4), np.int16), np.eye(4)).to_filename(path)


raw = {"Name": "t", "BIDSVersion": "1.8.0"}
deriv = {**raw, "DatasetType": "derivative", "GeneratedBy": [{"Name": "fmriprep"}]}
for parent in ("plain", "sub-pilot", "func", "andres-lab"):
    root = base / parent / "ds"
    for sub in ("01", "02"):
        nifti(root / f"sub-{sub}/anat/sub-{sub}_T1w.nii.gz")
    (root / "dataset_description.json").write_text(json.dumps(raw))
    out = root / "derivatives/fmriprep"
    nifti(out / "sub-01/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz")
    (out / "dataset_description.json").write_text(json.dumps(deriv))
    layout = BIDSLayout(root, validate=False, derivatives=str(out))
    t1w = layout.get(scope="raw", suffix="T1w", extension=".nii.gz")
    preproc = layout.get(scope="derivatives", suffix="T1w")
    print(f"{parent}: subjects {layout.get_subjects(scope='raw')};",
          f"raw T1w datatype {sorted({f.entities.get('datatype') for f in t1w})};",
          f"derivative res {[f.entities.get('res') for f in preproc]};",
          f"get(res='2') {len(layout.get(scope='derivatives', res='2'))}")  # fmt: skip
