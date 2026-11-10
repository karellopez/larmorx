# SPDX-License-Identifier: Apache-2.0
# SDC-45 (docs/upstream-reports/sdcflows-entities-from-parent-folders.md): the same dataset
# below a folder whose name holds no entity pattern, one that holds "den-" and one named as a datatype.
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
    ):  # fmt: skip
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
              "IntendedFor:", [f.metadata.get("IntendedFor") for e in found for f in e.sources])  # fmt: skip
    except Exception as err:
        print(parent, f"{type(err).__name__}: {err}")
