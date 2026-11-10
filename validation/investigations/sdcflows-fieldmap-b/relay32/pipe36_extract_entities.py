# SPDX-License-Identifier: Apache-2.0
# PIPE-36 (docs/upstream-reports/fmriprep-entities-from-parent-folders.md): fMRIPrep's
# extract_entities and get_sbrefs on a BOLD run's absolute path, below "plain", "golden-x" and "motion".
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
