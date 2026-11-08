# SPDX-License-Identifier: Apache-2.0
"""The sdcflows nodes fMRIPrep 25.2.6 ran in the oracle runs: inputs, durations, image grids.

    python -P oracle_nodes.py      (LARMORX_WORKSPACE: the workspace holding oracles/)

Reads every ``_report/report.rst`` of the field-map nodes part A of the field maps ports
(``bs_filter``, ``check_register``, ``phmap2rads``, ``calc_phdiff``, ``prelude``, ``compfmap``,
``fix_coeff``) and of fMRIPrep's reconstructions (``boldref_fmap``, ``fmap_recon``), and prints
their parameters and the grids of their NIfTI inputs that are still in the working directory.
"""

import os
import re
from pathlib import Path

import nibabel as nb

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
WANT = {
    "bs_filter",
    "check_register",
    "calc_phdiff",
    "compfmap",
    "fix_coeff",
    "prelude",
    "boldref_fmap",
    "fmap_recon",
    "phmap2rads0",
    "phmap2rads1",
}
FILE_KEYS = {
    "in_data",
    "in_mask",
    "in_file",
    "in_coeff",
    "in_coeffs",
    "fmap_ref",
    "magnitude_file",
    "phase_file",
    "mask_file",
    "target_ref_file",
    "fmap_ref_file",
    "mag_files",
    "fmap_files",
    "in_phases",
}
VALUE_KEYS = {
    "bs_spacing",
    "recenter",
    "zooms_min",
    "ridge_alpha",
    "extrapolate",
    "pe_dir",
    "in_meta",
    "metadata",
    "inverse",
    "transforms",
    "rot_thresh",
    "trans_thresh",
}


def section(text, name):
    m = re.search(name + r"\n-+\n(.*?)\n\n\n", text, re.S)
    return m.group(1) if m else ""


def main() -> None:
    for run in sorted(os.listdir(R)):
        for root, dirs, _files in os.walk(R / run):
            base = os.path.basename(root).lstrip("_")
            if base not in WANT or "_report" not in dirs:
                continue
            rep = (Path(root) / "_report" / "report.rst").read_text(encoding="utf-8")
            dur = re.search(r"\* duration : ([0-9.]+)", rep)
            rel = os.path.relpath(root, R / run / "work")
            print(f"== {run} {rel}" + (f"  ({float(dur.group(1)):.1f} s)" if dur else ""))
            for line in section(rep, "Execution Inputs").splitlines():
                line = line.strip()
                if not line.startswith("*"):
                    continue
                key, _, val = line[2:].partition(" : ")
                if key in FILE_KEYS:
                    for p in re.findall(r"/[^'\],\s]+\.nii(?:\.gz)?", val)[:3]:
                        hp = R / run / p[1:] if p.startswith("/work/") else Path(p)
                        try:
                            img = nb.load(hp)
                            z = tuple(round(float(v), 4) for v in img.header.get_zooms()[:3])
                            codes = "".join(nb.aff2axcodes(img.affine))
                            print(
                                f"   {key}: {os.path.basename(p)} {img.shape} {z} {codes} "
                                f"{img.get_data_dtype()}"
                            )
                        except Exception as exc:
                            print(
                                f"   {key}: {os.path.basename(p)} (not here: {type(exc).__name__})"
                            )
                elif key in VALUE_KEYS:
                    print(f"   {key}: {val[:200]}")


if __name__ == "__main__":
    main()
