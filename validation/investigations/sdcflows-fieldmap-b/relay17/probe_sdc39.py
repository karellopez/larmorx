# SPDX-License-Identifier: Apache-2.0
"""SDC-39 (rule D's measurement): sdcflows' ``brain_masker`` closes the inverted image with
``grey_closing(..., structure=ball(1))``, SciPy's non-flat closing (the ball's elements add 1,
the cube's corners 0), where a flat closing by the ball (``footprint=ball(1)``) is what a
grey-level closing with a ball means. On values in [0, 1] the two agree but for float32 roundings
(``x + 1 - 1``). This runs sdcflows' own ``brain_masker`` both ways (SciPy's ``grey_closing``
wrapped to take the structure as a flat footprint) on the nine ``brainextraction_wf`` inputs of
fMRIPrep's oracle runs (``check_skimage.py``'s ``masker`` section) and counts the voxels of the
closed image, the probability map and the mask that change.

Usage (newest environment, about 3 minutes): ``python -P probe_sdc39.py``.
"""

from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import check_skimage as cs  # noqa: E402


def main() -> int:
    import nibabel as nb
    import scipy.ndimage as ndi
    from sdcflows.utils.tools import brain_masker

    original = ndi.grey_closing
    closed_out: list[np.ndarray] = []

    def flat(input, size=None, footprint=None, structure=None, *args, **kw):
        if structure is not None and footprint is None:
            footprint, structure = np.asarray(structure) > 0, None
        out = original(input, size, footprint, structure, *args, **kw)
        closed_out.append(out)
        return out

    def keep(input, *args, **kw):
        out = original(input, *args, **kw)
        closed_out.append(out)
        return out

    rows = []
    for node in cs.masker_nodes():
        label = "/".join(node.parts[-6:-1])
        src = node / "clipper_post" / "clipped.nii.gz"
        res = []
        for wrap in (keep, flat):
            closed_out.clear()
            ndi.grey_closing = wrap
            try:
                with tempfile.TemporaryDirectory() as tmp:
                    _, prob, mask = brain_masker(str(src), os.path.join(tmp, "x_mask.nii.gz"))
                    res.append(
                        (
                            closed_out[0].copy(),
                            np.asanyarray(nb.load(prob).dataobj).copy(),
                            np.asanyarray(nb.load(mask).dataobj).copy(),
                        )
                    )
            finally:
                ndi.grey_closing = original
        (c0, p0, m0), (c1, p1, m1) = res
        row = (
            label,
            int(np.count_nonzero(c0 != c1)),
            int(np.count_nonzero(p0 != p1)),
            float(np.abs(p0 - p1).max()),
            int(np.count_nonzero(m0 != m1)),
            int(m0.sum()),
        )
        rows.append(row)
        print(
            f"{label}: closed {row[1]} voxels differ, probseg {row[2]} (max {row[3]:.3g}), "
            f"mask {row[4]} of {row[5]}"
        )
    changed = sum(r[4] for r in rows)
    print(
        f"VERDICT masks changed in {sum(r[4] > 0 for r in rows)} of {len(rows)}; {changed} voxels"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
