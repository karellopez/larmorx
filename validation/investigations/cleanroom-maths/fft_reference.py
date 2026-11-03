# SPDX-License-Identifier: Apache-2.0
"""How far FSL's FFT convolution and the clean-room original's direct sum each lie from the exact
result, on the evidence page's real call ``fslmaths mean -kernel sphere 21 -fmean out``
(FSL's -Tmean of ds005040's BOLD series in T1w space, 50 x 62 x 44 voxels of 3 x 3 x 3.3 mm: a
15 x 15 x 13 kernel, which FSL convolves by its FFT).

The exact result is computed here in float64 from ``specs/fslmaths.md``'s definition (§9: the
sphere's voxels where ``(i dx)^2 + (j dy)^2 + (k dz)^2 <= r^2``, in single precision; §10.2:
-fmean is the convolution of the image with the kernel, zero outside, divided by the same
convolution of ones): every voxel's sum over the kernel's voxels inside the image, divided by
their count. Each implementation's largest difference from it is given relative to the exact
result's largest magnitude (§21's measure), with where it lies (the count of kernel voxels
inside the image there).

The same functions make the evidence page's numbers (``larmorx_validation.evidence.fsl_maths``'s
families example). Usage, with ``PYTHONPATH`` holding the checkout's ``validation/src``:
python -P fft_reference.py <figures cache> [<cache of the original's entries>]
(the entries ``fsl-maths/families`` (FSL's run, through the oracle cache) and
``fsl-maths/original-families`` (the original's)).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.evidence import fsl_maths_original as fo


def main(argv: list[str]) -> int:
    cache = Path(argv[0])
    ocache = Path(argv[1]) if len(argv) > 1 else cache
    fam = cache / "fsl-maths" / "families"
    info = json.loads((fam / "RUN_INFO.json").read_text(encoding="utf-8"))
    mean_path = Path(info["inputs"]["mean"].split(" (sha256")[0])
    img = nib.load(str(mean_path))
    x = np.asarray(img.dataobj, dtype=np.float64)
    k = fo.sphere_kernel(21.0, img.header.get_zooms()[:3])
    exact, cnt = fo.exact_fmean(x, k)
    scale = float(np.abs(exact).max())
    print(f"kernel {k.shape}, {int(k.sum())} voxels; exact result's largest magnitude {scale:.6g}")
    runs = {
        "FSL 6.0.7.23 (FFT)": fam / "fsl-fmean-fft" / "fmean-fft.nii.gz",
        "original (direct sum)": ocache
        / "fsl-maths"
        / "original-families"
        / "original-fmean-fft"
        / "fmean-fft.nii.gz",
    }
    out = {}
    for name, p in runs.items():
        y = np.asarray(nib.load(str(p)).dataobj, dtype=np.float64)
        d = np.abs(y - exact)
        at = np.unravel_index(int(np.argmax(d)), d.shape)
        out[name] = y
        print(
            f"{name}: max |out - exact| / max |exact| {d.max() / scale:.3g} at voxel {tuple(map(int, at))} "
            f"(kernel voxels inside: {int(cnt[at])} of {int(k.sum())}); median |out - exact| "
            f"{np.median(d) / scale:.3g}; 99th percentile {np.percentile(d, 99) / scale:.3g}"
        )
    a, b = out.values()
    d = np.abs(a - b)
    at = np.unravel_index(int(np.argmax(d)), d.shape)
    print(
        f"original - FSL: max {d.max() / float(np.abs(a).max()):.3g} of FSL's largest value, at "
        f"voxel {tuple(map(int, at))} (kernel voxels inside: {int(cnt[at])})"
    )
    # what a sum in double rounded once to single would give (the exact result rounded): its
    # distance from FSL by §21's measure (the bound, and the share within 4 units in the last
    # place)
    from larmorx_validation.parity.fsl_maths import _ulps

    fsl32 = a.astype(np.float32)
    once = exact.astype(np.float32)
    ulp = _ulps(fsl32, once)
    print(
        f"the exact result rounded once to single - FSL: max "
        f"{np.abs(once.astype(np.float64) - a).max() / float(np.abs(a).max()):.3g} of FSL's "
        f"largest value, {np.mean(ulp <= 4):.2%} of the voxels within 4 ulp; the original: "
        f"{np.mean(_ulps(fsl32, b.astype(np.float32)) <= 4):.2%}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
