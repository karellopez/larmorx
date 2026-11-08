# SPDX-License-Identifier: Apache-2.0
"""Output names of 228 to 255 bytes in every AFNI clean-room original, against AFNI 26.3.05.

The replica part's testing reported (as a behaviour, 2026-11-07) that the clean-room originals
failed on output names of 228 to 255 bytes: their hidden working names (`.<name>.larmorx-...`)
passed the system's 255-byte limit. This runs 3dTshift, 3dAutomask, 3dUnifize and 3dvolreg,
AFNI's (through the oracle-output cache) and larmorx's original, with a `-prefix` whose file name
has 200, 227, 228, 240, 250 and 255 bytes, and reports each exit status and whether the file was
written.

    python -P longnames.py            # needs target/release/larmorx and blackbox.py's inputs
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import blackbox as bb  # noqa: E402

LENGTHS = (200, 227, 228, 240, 250, 255)


def programs(series: str, volume: str) -> dict[str, list[str]]:
    return {
        "3dTshift": ["-tpattern", "alt+z", series],
        "3dAutomask": [volume],
        "3dUnifize": [volume],
        "3dvolreg": [series],
    }


def main() -> int:
    import nibabel as nib

    from larmorx_validation import oracle_cache

    series = str(bb.INPUTS / "lps_f32.nii.gz")
    volume = bb.INPUTS / "lps_f32_vol0.nii.gz"
    if not volume.exists():
        img = nib.load(series)
        nib.save(nib.Nifti1Image(img.get_fdata()[..., 0].astype("float32"), img.affine), volume)
    root = Path(tempfile.mkdtemp(prefix="volreg-longnames_", dir=os.environ.get("VOLREG_WORK")))
    bad = 0
    for prog, args in programs(series, str(volume)).items():
        for n in LENGTHS:
            name = "p" * (n - 4) + ".nii"
            row = []
            for who in ("afni", "original"):
                d = root / prog / f"{who}-{n}"
                d.mkdir(parents=True)
                env = bb.clean_env(d)
                argv = ["-prefix", name, *args]
                if who == "afni":
                    exe = bb.WORKSPACE / f"oracles/afni-26.3.05/bin/{prog}"
                    r = oracle_cache.run(
                        [str(exe), *argv],
                        env=env,
                        cwd=d,
                        outputs=[d],
                        policy=oracle_cache.Policy(
                            threads=1,
                            family="afni",
                            nifti_extensions="AFNI's history: the date and the output's path",
                        ),
                        runner="afni-volreg-original-longnames",
                        timeout=600,
                    )
                    code = r.returncode
                else:
                    env["LARMORX_IMPLEMENTATION"] = "original"
                    r = subprocess.run([bb.LX, "afni", prog, *argv], cwd=d, env=env,
                                       capture_output=True, timeout=600)
                    code = r.returncode
                row.append((code, (d / name).is_file()))
            ok = row[0] == row[1]
            bad += not ok
            print(f"{prog:10} {n:3}  AFNI exit {row[0][0]} written {row[0][1]}   "
                  f"original exit {row[1][0]} written {row[1][1]}   {'same' if ok else 'DIFFERENT'}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
