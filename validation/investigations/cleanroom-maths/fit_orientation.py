# SPDX-License-Identifier: Apache-2.0
"""Where FSL's ``-fmeanu``/``-fmean`` lands an asymmetric kernel on an impulse: as stored (a true
convolution, FSL's FFT path) or mirrored along every axis (a correlation, its direct sum), for
the probes of findings 14 (FSL-192), made by ``fsl_outputs.py``:

```
boxed.sh 4 python -P fsl_outputs.py <dir> \\
  "args:or_23_odd:@probe:impulse23 -kernel file @probe:asym13x11x9 -fmeanu out" \\
  "args:or_25_odd:@probe:impulse25 -kernel file @probe:asym13x11x9 -fmeanu out" \\
  "args:or_23_odd_fmean:@probe:impulse23 -kernel file @probe:asym13x11x9 -fmean out" \\
  "args:or_25_odd_fmean:@probe:impulse25 -kernel file @probe:asym13x11x9 -fmean out" \\
  "args:or_15_even:@probe:impulse -kernel file @probe:asym14x12x10 -fmeanu out" \\
  "args:or_67_even:@probe:impulse67 -kernel file @probe:asym14x12x10 -fmeanu out"
python -P fit_orientation.py <dir>
```

Prints, per probe, whether §10.2's criterion takes FSL's FFT and the largest error of each
orientation relative to the normalised kernel's largest value (one of them near 1e-7, the other
near 1).
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

PROBES = (
    ("or_23_odd", 23, (13, 11, 9)),
    ("or_25_odd", 25, (13, 11, 9)),
    ("or_23_odd_fmean", 23, (13, 11, 9)),
    ("or_25_odd_fmean", 25, (13, 11, 9)),
    ("or_15_even", 15, (14, 12, 10)),
    ("or_67_even", 67, (14, 12, 10)),
)


def fft_switch(nvox: int, nker: int) -> bool:
    """§10.2's criterion, in single precision."""
    f = np.float32
    return bool(f(nvox) * f(nker) > f(44) * f(2 * nvox) * f(np.log2(f(2 * nvox))))


def main(argv: list[str]) -> int:
    import nibabel as nib

    base = Path(argv[0])
    for case, n, ks in PROBES:
        d = base / case
        if not d.exists():
            print(f"{case}: not made (run fsl_outputs.py first)")
            continue
        out = np.asarray(nib.load(str(d / "out.nii.gz")).dataobj).astype(np.float64)
        k = np.asarray(nib.load(str(d / f"probe_asym{ks[0]}x{ks[1]}x{ks[2]}.nii.gz")).dataobj)
        kn = k.astype(np.float64) / k.astype(np.float64).sum()
        p = np.array([n // 2] * 3)
        c = (np.array(ks) - 1) // 2
        errs = {}
        for label, sign in (("as stored (convolution)", 1), ("mirrored (correlation)", -1)):
            err = 0.0
            for i in np.ndindex(*ks):
                q = p + sign * (np.array(i) - c)
                if (q < 0).any() or (q >= n).any():
                    continue
                err = max(err, abs(out[tuple(q)] - kn[i]) / kn.max())
            errs[label] = err
        print(
            f"{case}: {n}^3 voxels, kernel {ks}, FFT by §10.2: {fft_switch(n**3, int(np.prod(ks)))}; "
            + ", ".join(f"{a} {b:.2g}" for a, b in errs.items())
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
