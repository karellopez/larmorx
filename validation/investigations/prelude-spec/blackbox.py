# SPDX-License-Identifier: Apache-2.0
"""Black-box runs of a ``prelude`` program for ``specs/prelude.md``: its messages, exit statuses,
the files it writes and their headers, on inputs generated here (seeded, numpy only).

Written from the spec and from runs of the program, never from FSL's source: a clean-room
implementer may run it, against FSL's binary or against its own original, and compare the two
reports line for line::

    python -I blackbox.py <work dir> <program> [<program word>...]

e.g. ``blackbox.py /tmp/bb-fsl $FSLDIR/bin/prelude`` (FSLDIR set, FSL off the PATH) or
``blackbox.py /tmp/bb-lx larmorx fsl prelude``. The report goes to standard output: for each
case its arguments, the exit status, standard output and standard error (FSL's usage text
replaced by ``<usage>`` after the first case that prints it) and each file written with its
header fields and a summary of its values. The work directory is emptied first.
"""

from __future__ import annotations

import gzip
import os
import shutil
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

NX, NY, NZ = 20, 16, 8


def truth(scale: float = 1.0) -> np.ndarray:
    """A smooth phase (radians) whose range spans several turns: x fastest in the file."""
    z, y, x = np.meshgrid(np.arange(NZ), np.arange(NY), np.arange(NX), indexing="ij")
    t = 0.06 * ((x - 9.5) ** 2 + 1.3 * (y - 7.5) ** 2) + 0.4 * (z - 3.5) - 2.0
    return (scale * t).transpose(2, 1, 0)  # to (x, y, z)


def wrapped(t: np.ndarray) -> np.ndarray:
    return np.angle(np.exp(1j * t)).astype(np.float32)


def head() -> np.ndarray:
    x, y, z = np.meshgrid(np.arange(NX), np.arange(NY), np.arange(NZ), indexing="ij")
    return ((x - 9.5) / 8.5) ** 2 + ((y - 7.5) / 6.5) ** 2 + ((z - 3.5) / 4.0) ** 2 <= 1.0


def magnitude(rng: np.random.Generator) -> np.ndarray:
    m = np.where(head(), 1000.0, 20.0) + rng.normal(0, 15, (NX, NY, NZ))
    return np.abs(m).astype(np.float32)


AFFINE = np.diag([-2.0, 2.0, 3.0, 1.0])
AFFINE[:3, 3] = [19.0, -15.0, -10.5]


def save(path: Path, data: np.ndarray, affine=AFFINE, **fields) -> None:
    img = nib.Nifti1Image(data, affine)
    h = img.header
    h.set_qform(affine, 1)
    h.set_sform(affine, 1)
    h.set_xyzt_units("mm", "sec")
    if data.ndim == 4:
        z = list(h.get_zooms())
        z[3] = fields.pop("tr", 2.5)
        h.set_zooms(z)
    for k, v in fields.items():
        h[k] = v
    nib.save(img, str(path))


def make_inputs(d: Path) -> None:
    rng = np.random.default_rng(20261106)
    t = truth()
    save(d / "ph.nii.gz", wrapped(t))
    save(d / "mag.nii.gz", magnitude(rng))
    save(d / "mask.nii.gz", head().astype(np.float32))
    one = np.zeros((NX, NY, NZ), np.float32)
    one[9, 7, 3] = 1.0
    save(d / "onevox.nii.gz", one)
    s3 = [1.0, 0.8, 1.2]
    save(d / "ph3.nii.gz", np.stack([wrapped(t * s) for s in s3], axis=3))
    save(d / "mag3.nii.gz", np.stack([magnitude(rng) for _ in s3], axis=3))
    save(d / "range.nii.gz", t.astype(np.float32))
    save(d / "ph2pi.nii.gz", (wrapped(t) + np.float32(np.pi)).astype(np.float32))
    c = (magnitude(rng) * np.exp(1j * t)).astype(np.complex64)
    save(d / "cplx.nii.gz", c)
    save(
        d / "phhdr.nii.gz",
        wrapped(t),
        cal_min=-3.5,
        cal_max=3.5,
        intent_code=2,
        intent_p1=7.0,
        aux_file=b"aux-of-phase",
        descrip=b"input descrip",
    )
    other = AFFINE.copy()
    other[:3, 3] += [4.0, 2.0, 0.0]
    save(d / "magother.nii.gz", magnitude(rng), affine=other, cal_max=999.0)
    for name, a in (("nph2pi", wrapped(t) + np.float32(np.pi)), ("nrange", t)):
        a = a.astype(np.float32)
        a[0, 0, 0] = np.nan
        save(d / f"{name}.nii.gz", a)
    # an image wider than 64 voxels (x), its phase a ramp along x with wraps
    x, y, z = np.meshgrid(np.arange(70), np.arange(10), np.arange(4), indexing="ij")
    tw = 0.25 * x + 0.3 * y - 0.2 * z
    save(d / "wideph.nii.gz", wrapped(tw))
    hw = ((x - 34.5) / 33.0) ** 2 + ((y - 4.5) / 4.6) ** 2 <= 1.0
    save(d / "widemag.nii.gz", np.where(hw, 900.0, 5.0).astype(np.float32))


CASES: list[tuple[str, list[str], dict[str, str | None]]] = [
    ("help", ["-h"], {}),
    ("no-arguments", [], {}),
    ("no-output", ["-p", "ph", "-a", "mag"], {}),
    ("complex-and-phase", ["-c", "cplx", "-p", "ph", "-o", "o"], {}),
    ("phase-without-abs", ["-p", "ph", "-o", "o"], {}),
    ("abs-without-phase", ["-a", "mag", "-o", "o"], {}),
    ("n1", ["-p", "ph", "-a", "mag", "-o", "o", "-n", "1"], {}),
    ("n-negative", ["-p", "ph", "-a", "mag", "-o", "o", "-n", "-3"], {}),
    ("unknown-option", ["-p", "ph", "-a", "mag", "-o", "o", "--bogus"], {}),
    ("unknown-short", ["-p", "ph", "-a", "mag", "-o", "o", "-q"], {}),
    ("bad-number", ["-p", "ph", "-a", "mag", "-o", "o", "-n", "abc", "-v"], {}),
    ("n-2.5", ["-p", "ph", "-a", "mag", "-o", "o", "-n", "2.5"], {}),
    ("n-hex", ["-p", "ph", "-a", "mag", "-o", "o", "-n", "0x3", "-v"], {}),
    ("n-octal", ["-p", "ph", "-a", "mag", "-o", "o", "-n", "010", "-v"], {}),
    ("n-missing", ["-p", "ph", "-a", "mag", "-o", "o", "-n"], {}),
    ("n-repeated", ["-p", "ph", "-a", "mag", "-o", "o", "-n", "3", "-n", "4", "-v"], {}),
    ("thresh-bad", ["-p", "ph", "-a", "mag", "-o", "o", "-t", "abc"], {}),
    ("thresh-long", ["-p", "ph", "-a", "mag", "-o", "o", "--thresh=1e9", "-v"], {}),
    ("stray-word", ["-p", "ph", "-a", "mag", "-o", "o", "stray"], {}),
    ("missing-file", ["-p", "nothere", "-a", "mag", "-o", "o"], {}),
    ("missing-magnitude", ["-p", "ph", "-a", "nothere", "-o", "o", "-v"], {}),
    ("missing-mask", ["-p", "ph", "-a", "mag", "-m", "nothere", "-o", "o", "-v"], {}),
    ("no-input", ["-o", "o"], {}),
    ("complex-empty-name", ["--complex=", "-o", "o"], {}),
    ("range", ["-p", "range", "-a", "mag", "-o", "o", "-v"], {}),
    ("range-third-volume", ["-p", "ph3x", "-a", "mag3", "-o", "o", "-v"], {}),
    ("long-option-space", ["-p", "ph3", "-a", "mag3", "-o", "o", "--start", "1"], {}),
    ("start-outside", ["-p", "ph3", "-a", "mag3", "-o", "o", "--start=5"], {}),
    ("start-negative", ["-p", "ph3", "-a", "mag3", "-o", "o", "--start=-1"], {}),
    ("end-outside", ["-p", "ph3", "-a", "mag3", "-o", "o", "--end=7", "-v"], {}),
    ("end-before-start", ["-p", "ph3", "-a", "mag3", "-o", "o", "--start=2", "--end=1",
                          "-r", "raw"], {}),
    ("verbose-with-value", ["-p", "ph", "-a", "mag", "-o", "o", "--verbose=no"], {}),
    ("grouped-flags", ["-p", "ph", "-a", "mag", "-o", "o", "-fv"], {}),
    ("grouped-value", ["-p", "ph", "-a", "mag", "-o", "o", "-vn", "3"], {}),
    ("help-with-others", ["-p", "ph", "-a", "mag", "-o", "o", "-h"], {}),
    ("outtype-unset", ["-p", "ph", "-a", "mag", "-o", "o", "-v"], {"FSLOUTPUTTYPE": None}),
    ("outtype-bogus", ["-p", "ph", "-a", "mag", "-o", "o", "-v", "--savemask=m"],
     {"FSLOUTPUTTYPE": "bogus"}),
    ("out-missing-dir", ["-p", "ph", "-a", "mag", "-o", "nodir/o"], {}),
    ("out-extension", ["-p", "ph", "-a", "mag", "-o", "o.nii", "--savemask=m.hdr"], {}),
    ("out-nifti", ["-p", "ph", "-a", "mag", "-o", "o.nii.gz"], {"FSLOUTPUTTYPE": "NIFTI"}),
    ("verbose", ["-p", "ph", "-a", "mag", "-o", "o", "-v"], {}),
    ("verbose-2pi-mask", ["-p", "ph2pi", "-a", "mag", "-m", "mask", "-o", "o", "-v"], {}),
    ("verbose-ramps", ["-p", "ph", "-a", "mag", "-o", "o", "-v", "--removeramps", "-r", "raw"],
     {}),
    ("verbose-slices", ["-p", "ph", "-a", "mag", "-o", "o", "-v", "-s"], {}),
    ("verbose-labelslices", ["-p", "ph", "-a", "mag", "-o", "o", "-v", "--labelslices",
                             "-l", "lab"], {}),
    ("series", ["-p", "ph3", "-a", "mag3", "-o", "o", "-v", "--savemask=m", "-l", "lab",
                "-r", "raw"], {}),
    ("series-start", ["-p", "ph3", "-a", "mag3", "-o", "o", "--start=1"], {}),
    ("mask-one-voxel", ["-p", "ph", "-a", "mag", "-m", "onevox", "-o", "o", "-v"], {}),
    ("empty-mask", ["-p", "ph", "-a", "mag", "-o", "o", "-t", "1e9", "-v", "-l", "lab"], {}),
    ("complex-as-phase", ["-p", "cplx", "-a", "mag", "-o", "o"], {}),
    ("magnitude-shorter", ["-p", "ph3", "-a", "mag", "-o", "o", "-v"], {}),
    ("mask-series", ["-p", "ph3", "-a", "mag3", "-m", "mask", "-o", "o", "-v"], {}),
    ("nan-first-phase-2pi", ["-p", "nph2pi", "-a", "mag", "-o", "o", "-v"], {}),
    ("nan-first-phase-range", ["-p", "nrange", "-a", "mag", "-o", "o", "-v"], {}),
    ("wide-auto-2d", ["-p", "wideph", "-a", "widemag", "-o", "o", "-v", "-l", "lab"], {}),
    ("wide-force3d", ["-p", "wideph", "-a", "widemag", "-o", "o", "-v", "-f", "-l", "lab"], {}),
    ("headers", ["-p", "phhdr", "-a", "magother", "-o", "o", "--savemask=m", "-l", "lab",
                 "-r", "raw"], {}),
    ("headers-complex", ["-c", "cplx", "-o", "o", "--savemask=m", "-l", "lab", "-r", "raw"],
     {}),
]

FIELDS = ("datatype", "dim", "pixdim", "qform_code", "sform_code", "cal_min", "cal_max",
          "intent_code", "intent_p1", "xyzt_units", "scl_slope", "scl_inter", "aux_file",
          "descrip", "vox_offset")


def describe(path: Path) -> list[str]:
    img = nib.load(str(path))
    h = img.header
    out = [f"  file {path.name}"]
    for f in FIELDS:
        v = h[f]
        if f == "dim":
            v = v[: int(v[0]) + 1]
        if f == "pixdim":
            v = np.round(v[: int(h["dim"][0]) + 1], 6)
        out.append(f"    {f} = {np.asarray(v).tolist()}")
    out.append(f"    sform = {np.round(h.get_sform(), 4).tolist()}")
    a = np.asarray(img.dataobj)
    if np.iscomplexobj(a):
        a = np.abs(a)
    fin = a[np.isfinite(a)]
    out.append(
        f"    values: shape {list(a.shape)} min {fin.min() if fin.size else 'nan'} "
        f"max {fin.max() if fin.size else 'nan'} nonzero {int(np.count_nonzero(a))} "
        f"nan {int(np.isnan(a).sum())}"
    )
    return out


def main() -> int:
    work = Path(sys.argv[1]).resolve()
    program = sys.argv[2:]
    inputs = work / "inputs"
    if work.exists():
        shutil.rmtree(work)
    inputs.mkdir(parents=True)
    make_inputs(inputs)
    # ph3x: three volumes, the third exceeding the range
    a = np.asarray(nib.load(str(inputs / "ph3.nii.gz")).dataobj).copy()
    a[..., 2] = truth().astype(np.float32)
    save(inputs / "ph3x.nii.gz", a)
    env0 = {k: v for k, v in os.environ.items() if not k.startswith("LARMORX_")}
    env0["PATH"] = ":".join(p for p in env0.get("PATH", "").split(":") if "/fsl" not in p)
    env0["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    usage: str | None = None
    for name, args, env_over in CASES:
        d = work / name
        d.mkdir()
        for f in inputs.iterdir():
            os.symlink(f, d / f.name)
        env = dict(env0)
        for k, v in env_over.items():
            if v is None:
                env.pop(k, None)
            else:
                env[k] = v
        r = subprocess.run(
            [*program, *args], cwd=d, env=env, capture_output=True, text=True, check=False
        )
        print(f"== {name}: {' '.join(args)}" + (f"  (env {env_over})" if env_over else ""))
        print(f"  exit {r.returncode}")
        err = r.stderr
        if name == "help":
            usage = err if err else r.stdout
        for label, text in (("stdout", r.stdout), ("stderr", err)):
            if usage and usage in text and name != "help":
                text = text.replace(usage, "<usage>")
            if text:
                print(f"  {label}:")
                for line in text.splitlines():
                    print(f"    | {line}")
        for f in sorted(d.rglob("*")):
            if f.is_symlink() or f.is_dir():
                continue
            if f.name.endswith((".nii", ".nii.gz", ".hdr")):
                print("\n".join(describe(f)))
            elif f.name.endswith(".img"):
                print(f"  file {f.name}")
            else:
                op = gzip.open if f.name.endswith(".gz") else open
                with op(f, "rb") as fh:
                    print(f"  file {f.name} ({len(fh.read())} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
