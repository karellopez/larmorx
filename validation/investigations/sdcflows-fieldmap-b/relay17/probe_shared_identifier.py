# SPDX-License-Identifier: Apache-2.0
"""PIPE-32: fMRIPrep's ``map_fieldmap_estimation`` (25.2.7's source, as ``check_estimators.py``
loads it) for two participants of one run that share a ``B0FieldIdentifier`` (each a PEPOLAR
pair named ``pepolar``, its BOLD run's ``B0FieldSource``), in one process without clearing
sdcflows' registry, as fMRIPrep's ``init_fmriprep_wf`` calls it subject after subject; and
larmorx's ``map_fieldmap_estimation`` with ``upstream_defects="reproduce"`` (the same error)
and by default (both participants mapped, a warning).

Usage (newest environment): ``python -P probe_shared_identifier.py <scratch folder>``. Prints
``OK``/``FAIL`` lines and ``ALL OK`` at the end.
"""

from __future__ import annotations

import json
import logging
import sys
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent / "relay11"))
import check_estimators as ce  # noqa: E402
import estimators_dump as dump  # noqa: E402


def tree(root: Path) -> Path:
    for sub in ("01", "02"):
        for d, pe in (("AP", "j-"), ("PA", "j")):
            p = root / f"sub-{sub}/fmap/sub-{sub}_dir-{d}_epi.nii.gz"
            p.parent.mkdir(parents=True, exist_ok=True)
            nb.Nifti1Image(np.zeros((4, 4, 4), np.int16), np.eye(4)).to_filename(p)
            meta = {"PhaseEncodingDirection": pe, "TotalReadoutTime": 0.05}
            meta["B0FieldIdentifier"] = "pepolar"
            p.with_name(p.name.replace(".nii.gz", ".json")).write_text(json.dumps(meta))
        p = root / f"sub-{sub}/func/sub-{sub}_task-rest_bold.nii.gz"
        p.parent.mkdir(parents=True, exist_ok=True)
        nb.Nifti1Image(np.zeros((4, 4, 4, 2), np.int16), np.eye(4)).to_filename(p)
        meta = {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2}
        meta["B0FieldSource"] = "pepolar"
        p.with_name(p.name.replace(".nii.gz", ".json")).write_text(json.dumps(meta))
    (root / "dataset_description.json").write_text('{"Name": "t", "BIDSVersion": "1.8.0"}')
    return root


def main() -> int:
    from bids.layout import BIDSLayout
    from sdcflows import fieldmaps as sfm

    import larmorx.sdcflows as ls
    from larmorx.pipelines.larmorprepx import estimators as le

    root = tree(Path(sys.argv[1]) / "shared-id")
    logger = logging.getLogger("probe.fmriprep")
    fns = dump.fmriprep_functions(
        str(ce.TAGS / "fmriprep-25.2.7" / "fmriprep" / "workflows" / "base.py"), logger
    )
    ok = True

    def check(name: str, cond: bool, detail: str) -> None:
        nonlocal ok
        ok &= cond
        print(f"{'OK' if cond else 'FAIL'} {name}: {detail}")

    def run(fn, lay, **kw):
        out = []
        for sub in ("01", "02"):
            bold = [[str(root / f"sub-{sub}/func/sub-{sub}_task-rest_bold.nii.gz")]]
            try:
                ests, emap = fn(lay, sub, None, bold, False, False, False, None, **kw)
                out.append(([e.bids_id for e in ests], {Path(k).name: v for k, v in emap.items()}))
            except Exception as e:  # the error is the result
                out.append(f"{type(e).__name__}: {e}")
        return out

    sfm.clear_registry()
    a = run(fns["map_fieldmap_estimation"], BIDSLayout(str(root), validate=False))
    ls.clear_registry()
    lay = le.fmriprep_layout(root, ["01", "02"])
    b = run(le.map_fieldmap_estimation, lay, upstream_defects="reproduce")
    check("fMRIPrep stops at the second participant", isinstance(a[1], str), repr(a))
    check("reproduce = fMRIPrep", a == b, repr(b))
    ls.clear_registry()
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        c = run(le.map_fieldmap_estimation, lay)
    named = [x.message.entry for x in w if hasattr(x.message, "entry")]
    want = [
        (["pepolar"], {"sub-01_task-rest_bold.nii.gz": "pepolar"}),
        (["pepolar"], {"sub-02_task-rest_bold.nii.gz": "pepolar"}),
    ]
    check("fix: both participants mapped", c == want, repr(c))
    check("fix: one PIPE-32 warning", named == ["PIPE-32"], repr(named))
    check("fix: the first participant's sources stay in the shared registry",
          all("sub-01" in s for s in ls.REGISTRY.estimators["pepolar"]),
          repr(ls.REGISTRY.estimators["pepolar"]))  # fmt: skip
    print("ALL OK" if ok else "FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
