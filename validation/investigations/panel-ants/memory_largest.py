# SPDX-License-Identifier: Apache-2.0
"""Peak memory and time of each ANTs tool's largest call on the data panel (the 7 T 0.8 mm
MP2RAGE, ds004958's 600-volume multiband-8 BOLD, ds002603's 588-volume multiband-4 run): ANTs
2.6.5 and larmorx (both D13 modes) at one thread, each its own process, through
``memory_real.py`` (``/usr/bin/time -v``); the results in ``<out>/<case>/memory.json`` and a
table ``<out>/memory.md``.

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P memory_largest.py <out dir> [case ...]

Run it in a memory box on a quiet machine (``scripts/orchestration/boxed.sh``).
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def cases() -> dict[str, tuple[str, bool, list[str]]]:
    """case -> (program, CORE-MATH preloaded for ANTs, arguments with ``{out}``)."""
    import larmorx_testdata as td

    from larmorx_validation.parity import fmriprep_nodes
    from larmorx_validation.parity import panel_images as P

    def f(image) -> str:
        return str(td.get(image.path))

    root = fmriprep_nodes.root()
    mb8 = root / "ds004958-sub-fsm64ps-noreconall" / "out" / "sub-fsm64ps" / "func"
    seven = root / "ds005454-sub-16-noreconall" / "out" / "sub-16" / "anat"
    mni1 = str(
        td.get("templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-01_T1w.nii.gz")
    )
    atropos = next(c for c in fmriprep_nodes.calls("Atropos") if c.run.startswith("ds005454"))
    a_args = list(atropos.args)
    i = a_args.index("--output")
    a_args[i + 1] = "[{out}/seg.nii.gz,{out}/POSTERIOR_%02d.nii.gz]"
    return {
        "imagemath-truncate-7t": ("ImageMath", False, ["3", "{out}/o.nii.gz",
            "TruncateImageIntensity", f(P.MP2RAGE_7T), "0.01", "0.999", "256"]),
        "imagemath-m-bold-mb8": ("ImageMath", False, ["4", "{out}/o.nii.gz", "m",
            f(P.BOLD_MB8), "2"]),
        "threshold-otsu-7t": ("ThresholdImage", False, ["3", f(P.MP2RAGE_7T),
            "{out}/o.nii.gz", "Otsu", "4"]),
        "multiply-bold-mb4": ("MultiplyImages", False, ["4", f(P.BOLD_MB4_PHILIPS), "0.5",
            "{out}/o.nii.gz"]),
        "smooth-4d-bold-mb4": ("SmoothImage", False, ["4", f(P.BOLD_MB4_PHILIPS),
            "2x2x2x0.55", "{out}/o.nii.gz", "1"]),
        "resample-bspline-7t": ("ResampleImage", False, ["3", f(P.MP2RAGE_7T),
            "{out}/o.nii.gz", "1", "0", "4"]),
        "resample-spacing-7t": ("ResampleImageBySpacing", False, ["3", f(P.MP2RAGE_7T),
            "{out}/o.nii.gz", "2", "2", "2"]),
        "n4-final-7t": ("N4BiasFieldCorrection", True, ["-d", "3", "--bspline-fitting",
            "[200]", "--convergence", "[50x50x50x50x50,1e-07]", "--shrink-factor", "4",
            "--input-image", f(P.MP2RAGE_7T), "--output",
            "[{out}/c.nii.gz,{out}/b.nii.gz]"]),
        "denoise-7t": ("DenoiseImage", True, ["-i", f(P.MP2RAGE_7T), "-n", "Rician", "-o",
            "[{out}/d.nii.gz,{out}/n.nii.gz]", "-s", "1"]),
        "atropos-fmriprep-7t": ("Atropos", True, a_args),
        "apply-bold-mb8-to-t1w": ("antsApplyTransforms", True, ["-d", "3", "-e", "3",
            "-i", str(next(mb8.glob("*_task-rest_desc-preproc_bold.nii.gz"))),
            "-r", str(next(mb8.glob("*_task-rest_space-T1w_boldref.nii.gz"))),
            "-t", str(next(mb8.glob("*_from-boldref_to-T1w_mode-image_desc-coreg_xfm.txt"))),
            "-n", "LanczosWindowedSinc", "--float", "1", "-o", "{out}/o.nii.gz"]),
        "apply-7t-to-mni": ("antsApplyTransforms", True, ["-d", "3", "-i", f(P.MP2RAGE_7T),
            "-r", mni1,
            "-t", str(next(seven.glob("*_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"))),
            "-t", str(next(seven.glob("*_acq-mp2rage_from-orig_to-T1w_mode-image_xfm.txt"))),
            "-n", "LanczosWindowedSinc", "-o", "{out}/o.nii.gz"]),
    }  # fmt: skip


def main() -> int:
    out = Path(sys.argv[1])
    want = sys.argv[2:]
    rows = []
    for name, (program, crmath, args) in cases().items():
        if want and name not in want:
            continue
        work = out / name
        cmd = [sys.executable, "-P", str(HERE / "memory_real.py"), str(work)]
        if crmath:
            cmd.append("--crmath")
        subprocess.run([*cmd, program, "--", *args], check=False)
        doc = json.loads((work / "memory.json").read_text(encoding="utf-8"))
        r = doc["runs"]
        worst = max(r["larmorx-reproduce"]["peak_mib"], r["larmorx-fix"]["peak_mib"])
        rows.append(
            f"| `{name}` | {program} | {r['ANTs']['peak_mib']:.0f} | "
            f"{r['larmorx-reproduce']['peak_mib']:.0f} | {r['larmorx-fix']['peak_mib']:.0f} | "
            f"{worst - r['ANTs']['peak_mib']:+.0f} | {r['ANTs']['seconds']:.1f} | "
            f"{r['larmorx-reproduce']['seconds']:.1f} | {doc['detail']} |"
        )
    table = [
        "| Case | Program | ANTs peak (MiB) | larmorx reproduce | larmorx default | larmorx − ANTs "
        "| ANTs s | larmorx s | Outputs (reproduce vs ANTs"
        " (CORE-MATH where marked)) |",
        "|---|---|---|---|---|---|---|---|---|",
        *rows,
    ]
    (out / "memory.md").write_text("\n".join(table) + "\n", encoding="utf-8")
    print("\n".join(table))
    return 0


if __name__ == "__main__":
    sys.exit(main())
