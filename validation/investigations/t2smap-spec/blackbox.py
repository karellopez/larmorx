# SPDX-License-Identifier: Apache-2.0
"""Black-box runs of a ``t2smap`` program for ``specs/t2smap.md``: its messages, exit statuses,
the files it writes and their headers, on multi-echo series generated here (seeded numpy).

Written from the spec and from runs of the program, never from tedana's source: a clean-room
implementer may run it, against tedana's program or against its own original, and compare the
two reports line for line::

    python -I blackbox.py <work dir> <program> [<program word>...]

e.g. ``blackbox.py /tmp/bb-ted <workspace>/oracles/inputs/tedana-t2smap-replica/launcher-newest/t2smap``
(tedana 26.0.3 in the newest environment) or ``blackbox.py /tmp/bb-lx larmorx tedana t2smap``.
The report goes to standard output: for each case its arguments, the exit status, standard
output and standard error (the work directory written ``<W>``, the Python installation's
``site-packages`` directory ``<site>``, a traceback's stack left out: its first and last lines
kept) and each file written with its header fields and a summary of its values. The work
directory is emptied first; every case's output directory is kept for ``model.py``.

The environment of every run (the spec's §12.1): ``COLUMNS=80``, ``TQDM_DISABLE=1``,
``OPENBLAS_NUM_THREADS=1``, ``OPENBLAS_CORETYPE=Haswell`` and numpy's AVX-512 kernels off
(``NPY_DISABLE_CPU_FEATURES``), so that the numbers are those of the spec's reference setup.
"""

from __future__ import annotations

import gzip
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

NX, NY, NZ, NT = 16, 14, 8, 6
TES_MS = (12, 28, 44)
NPY_OFF = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"
AFFINE = np.array(
    [[-3.0, 0.0, 0.0, 22.5], [0.0, 3.0, 0.0, -19.5], [0.0, 0.0, 3.5, -12.25], [0, 0, 0, 1]]
)


def phantom() -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """S0, T2* (ms) and the head's mask on the grid; (x, y, z) arrays."""
    x, y, z = np.meshgrid(np.arange(NX), np.arange(NY), np.arange(NZ), indexing="ij")
    r = ((x - 7.5) / 6.5) ** 2 + ((y - 6.5) / 5.5) ** 2 + ((z - 3.5) / 3.6) ** 2
    head = r <= 1.0
    s0 = np.where(head, 1000.0 + 25.0 * x - 10.0 * y, 3.0)
    t2 = np.where(head, 18.0 + 3.0 * y + 2.0 * z, 9.0)
    return s0, t2, head


def series() -> list[np.ndarray]:
    """Three echoes (x, y, z, t) float64: S0 times the decay as a product of per-ms factors
    (no transcendental function: the same bits on every machine), with seeded noise, a dropout
    band (x <= 2: the last echo at a tenth) and rising voxels (y = 0 row: the last echo above the
    second)."""
    s0, t2, head = phantom()
    rng = np.random.default_rng(20261108)
    factor = 1.0 - 1.0 / t2
    echoes = []
    for te in TES_MS:
        decay = np.ones_like(factor)
        for _ in range(te):
            decay = decay * factor
        e = s0 * decay
        echoes.append(e)
    echoes[2] = np.where(head & (np.arange(NX)[:, None, None] <= 2), echoes[2] * 0.1, echoes[2])
    rising = head & (np.arange(NY)[None, :, None] == 0)
    echoes[2] = np.where(rising, echoes[1] * 1.2, echoes[2])
    out = []
    for e in echoes:
        noise = rng.standard_normal((NX, NY, NZ, NT))
        out.append(e[..., None] * (1.0 + 0.02 * noise) + 0.5 * np.abs(noise))
    return out


def save(arr: np.ndarray, path: Path, dtype=np.float32, affine=AFFINE, slope=None) -> None:
    img = nib.Nifti1Image(np.asarray(arr), affine)
    img.header.set_xyzt_units("mm", "sec")
    zooms = (3.0, 3.0, 3.5, 2.0)[: arr.ndim]
    img.header.set_zooms(zooms)
    img.set_data_dtype(dtype)
    if slope is not None:
        img.header.set_slope_inter(slope, 0.0)
    img.to_filename(str(path))


def make_inputs(d: Path) -> None:
    d.mkdir(parents=True)
    ech = series()
    _, _, head = phantom()
    for i, e in enumerate(ech, 1):
        save(e, d / f"e{i}.nii.gz")
        q = np.round(e / 0.5)
        img = nib.Nifti1Image(q.astype(np.int16), AFFINE)
        img.header.set_xyzt_units("mm", "sec")
        img.header.set_zooms((3.0, 3.0, 3.5, 2.0))
        img.header.set_slope_inter(0.5, 0.0)
        img.to_filename(str(d / f"i16-e{i}.nii.gz"))
    save(np.concatenate(ech, axis=2), d / "zcat.nii.gz")
    save(ech[0][..., 0], d / "e1-3d.nii.gz")
    nan = ech[0].copy()
    nan[7, 6, 3, 2] = np.nan
    save(nan, d / "nan-e1.nii.gz")
    inf = ech[1].copy()
    inf[7, 6, 3, 2] = np.inf
    save(inf, d / "inf-e2.nii.gz")
    mask = np.zeros((NX, NY, NZ), dtype=np.uint8)
    x, y, z = np.meshgrid(np.arange(NX), np.arange(NY), np.arange(NZ), indexing="ij")
    big = ((x - 7.5) / 7.2) ** 2 + ((y - 6.5) / 6.2) ** 2 + ((z - 3.5) / 4.0) ** 2 <= 1.0
    mask[big] = 1
    save(mask, d / "mask.nii.gz", dtype=np.uint8)
    shifted = AFFINE.copy()
    shifted[0, 3] += 3.0
    save(mask, d / "mask-shifted.nii.gz", dtype=np.uint8, affine=shifted)
    del head


E3 = ["-d", "<I>/e1.nii.gz", "<I>/e2.nii.gz", "<I>/e3.nii.gz"]
TES = ["-e", "0.012", "0.028", "0.044"]
MASK = ["--mask", "<I>/mask.nii.gz"]

# (name, arguments, runs): `runs` > 1 runs the same command again in the same directory.
CASES: list[tuple[str, list[str], int]] = [
    ("help", ["-h"], 1),
    ("version", ["-v"], 1),
    ("no-arguments", [], 1),
    ("bad-choice", [*E3, *TES, "--fittype", "exp"], 1),
    ("missing-file", ["-d", "<I>/none.nii.gz", "-e", "0.012"], 1),
    ("bad-int", [*E3, *TES, "--dummy-scans", "1.5"], 1),
    ("loglin", [*E3, *TES, *MASK], 1),
    ("loglin-ms", [*E3, "-e", "12", "28", "44", *MASK], 1),
    ("te-mixed", [*E3, "-e", "0.012", "28", "44", *MASK], 1),
    ("te-negative", [*E3, "-e", "-0.012", "0.028", "0.044", *MASK], 1),
    ("te-inf", [*E3, "-e", "12", "28", "inf", *MASK], 1),
    ("te-count", [*E3, "-e", "0.012", "0.028", *MASK], 1),
    ("curvefit", [*E3, *TES, *MASK, "--fittype", "curvefit"], 1),
    (
        "curvefit-verbose-orig-prefix",
        [*E3, *TES, *MASK, "--fittype", "curvefit", "--verbose", "--convention", "orig",
         "--prefix", "sub-01"],
        1,
    ),
    ("ts-loglin", [*E3, *TES, *MASK, "--fitmode", "ts"], 1),
    ("ts-curvefit-verbose", [*E3, *TES, *MASK, "--fitmode", "ts", "--fittype", "curvefit",
                             "--verbose"], 1),
    ("paid", [*E3, *TES, *MASK, "--combmode", "paid"], 1),
    ("paid-ts", [*E3, *TES, *MASK, "--combmode", "paid", "--fitmode", "ts"], 1),
    ("no-mask", [*E3, *TES], 1),
    ("masktype-decay", [*E3, *TES, *MASK, "--masktype", "decay"], 1),
    ("masktype-none", [*E3, *TES, *MASK, "--masktype", "none"], 1),
    ("masktype-both", [*E3, *TES, *MASK, "--masktype", "dropout", "decay"], 1),
    ("dummy-scans", [*E3, *TES, *MASK, "--dummy-scans", "2"], 1),
    ("dummy-scans-all", [*E3, *TES, *MASK, "--dummy-scans", "6"], 1),
    ("exclude", [*E3, *TES, *MASK, "--exclude", "0,2:4"], 1),
    ("exclude-dummy", [*E3, *TES, *MASK, "--exclude", "0,1,4", "--dummy-scans", "2"], 1),
    ("exclude-dummy-all", [*E3, *TES, *MASK, "--exclude", "0,1", "--dummy-scans", "2"], 1),
    ("exclude-step", [*E3, *TES, *MASK, "--exclude", "0:4:2"], 1),
    ("exclude-open", [*E3, *TES, *MASK, "--exclude", "2:"], 1),
    ("exclude-negative", [*E3, *TES, *MASK, "--exclude", "-1"], 1),
    ("exclude-negative-range", [*E3, *TES, *MASK, "--exclude", "-2:1"], 1),
    ("exclude-text", [*E3, *TES, *MASK, "--exclude", "a"], 1),
    ("exclude-large", [*E3, *TES, *MASK, "--exclude", "7"], 1),
    ("exclude-equal", [*E3, *TES, *MASK, "--exclude", "6"], 1),
    ("exclude-all", [*E3, *TES, *MASK, "--exclude", "0:6"], 1),
    ("exclude-ts", [*E3, *TES, *MASK, "--exclude", "1", "--fitmode", "ts"], 1),
    ("zcat", ["-d", "<I>/zcat.nii.gz", *TES, *MASK], 1),
    ("int16", ["-d", "<I>/i16-e1.nii.gz", "<I>/i16-e2.nii.gz", "<I>/i16-e3.nii.gz", *TES, *MASK],
     1),
    ("n-independent-3", [*E3, *TES, *MASK, "--n-independent-echos", "3"], 1),
    ("n-independent-4", [*E3, *TES, *MASK, "--n-independent-echos", "4"], 1),
    ("n-independent-0", [*E3, *TES, *MASK, "--n-independent-echos", "0"], 1),
    ("debug-curvefit", [*E3, *TES, *MASK, "--fittype", "curvefit", "--debug"], 1),
    ("quiet", [*E3, *TES, *MASK, "--quiet"], 1),
    ("exists", [*E3, *TES, *MASK], 2),
    ("overwrite", [*E3, *TES, *MASK, "-f"], 2),
    ("nan-sample", ["-d", "<I>/nan-e1.nii.gz", "<I>/e2.nii.gz", "<I>/e3.nii.gz", *TES, *MASK], 1),
    ("nan-sample-decay", ["-d", "<I>/nan-e1.nii.gz", "<I>/e2.nii.gz", "<I>/e3.nii.gz", *TES,
                          *MASK, "--masktype", "decay"], 1),
    ("inf-sample", ["-d", "<I>/e1.nii.gz", "<I>/inf-e2.nii.gz", "<I>/e3.nii.gz", *TES, *MASK], 1),
    ("one-echo", ["-d", "<I>/e1.nii.gz", "-e", "0.012", *MASK], 1),
    ("two-echoes", ["-d", "<I>/e1.nii.gz", "<I>/e2.nii.gz", "-e", "0.012", "0.028", *MASK], 1),
    ("three-d", ["-d", "<I>/e1-3d.nii.gz", "<I>/e1-3d.nii.gz", "<I>/e1-3d.nii.gz", *TES, *MASK],
     1),
    ("mask-shifted", [*E3, *TES, "--mask", "<I>/mask-shifted.nii.gz"], 1),
    ("abbreviations", [*E3, *TES, "--mask=<I>/mask.nii.gz", "--fitt", "curvefit", "--comb=paid"],
     1),
    ("threads", [*E3, *TES, *MASK, "--fittype", "curvefit", "--n-threads", "2"], 1),
]


NO_OUT_DIR = {"help", "version", "no-arguments", "missing-file"}


def environment() -> dict[str, str]:
    env = {k: v for k, v in os.environ.items() if not k.startswith(("NPY_", "OPENBLAS_", "OMP_"))}
    env.update(
        COLUMNS="80",
        TQDM_DISABLE="1",
        OPENBLAS_NUM_THREADS="1",
        OPENBLAS_CORETYPE="Haswell",
        NPY_DISABLE_CPU_FEATURES=NPY_OFF,
    )
    return env


SITE = re.compile(r"\S*/site-packages/")
TRACE_FILE = re.compile(r'^  File "')


def normalise(text: str, work: Path) -> list[str]:
    """Paths written ``<W>``/``<site>``; a traceback's stack left out (first and last lines)."""
    text = text.replace(str(work), "<W>")
    text = SITE.sub("<site>/", text)
    lines = text.splitlines()
    out: list[str] = []
    in_trace = False
    for line in lines:
        if line.startswith("Traceback (most recent call last):"):
            out.append(line)
            out.append("  <stack>")
            in_trace = True
            continue
        if in_trace:
            if line.startswith(" ") or line == "":
                continue
            in_trace = False
        out.append(line)
    return out


def describe_image(path: Path) -> list[str]:
    img = nib.load(str(path))
    h = img.header
    data = np.asanyarray(img.dataobj)
    finite = data[np.isfinite(data)] if data.dtype.kind == "f" else data
    lines = [
        f"    class {type(img).__name__}, dtype {data.dtype}, shape {data.shape}",
        f"    zooms {tuple(float(z) for z in h.get_zooms())}, xyzt_units {int(h['xyzt_units'])}, "
        f"sform_code {int(h['sform_code'])}, qform_code {int(h['qform_code'])}, "
        f"descrip {h['descrip'].tobytes().rstrip(b'\x00')!r}",
        f"    slope {h['scl_slope']}, inter {h['scl_inter']}, pixdim0 {float(h['pixdim'][0])}, "
        f"intent {int(h['intent_code'])}",
        f"    srow_x {[float(v) for v in h['srow_x']]}",
    ]
    if finite.size:
        lines.append(
            f"    values: nonzero {int(np.count_nonzero(data))}, nan "
            f"{int(np.isnan(data).sum()) if data.dtype.kind == 'f' else 0}, min "
            f"{float(np.min(finite))!r}, max {float(np.max(finite))!r}"
        )
    with open(path, "rb") as f:
        magic = f.read(2)
    if magic == b"\x1f\x8b":
        with gzip.open(path) as g:
            g.read(1)
        raw = path.read_bytes()
        lines.append(f"    gzip: flags {raw[3]}, mtime {int.from_bytes(raw[4:8], 'little')}, "
                     f"xfl {raw[8]}")
    return lines


def describe_dir(out: Path, work: Path) -> list[str]:
    lines: list[str] = []
    if not out.exists():
        return ["  (no output directory)"]
    for p in sorted(out.rglob("*")):
        rel = p.relative_to(out)
        if p.is_dir():
            lines.append(f"  {rel}/ (directory, {len(list(p.iterdir()))} entries)")
            continue
        lines.append(f"  {rel}")
        name = p.name
        if name.endswith((".nii", ".nii.gz")):
            lines.extend(describe_image(p))
        elif name.endswith(".tsv"):
            text = p.read_text(encoding="utf-8").splitlines()
            lines.append(f"    {len(text)} lines; {text[0]!r}")
            lines.extend(f"    {t!r}" for t in text[1:3])
        elif name.endswith(".json"):
            obj = json.loads(p.read_text(encoding="utf-8"))
            if "GeneratedBy" in obj:
                gb = obj["GeneratedBy"][0]
                lines.append(f"    keys {sorted(obj)}; GeneratedBy keys {sorted(gb)}")
                lines.append(f"    Node keys {sorted(gb.get('Node', {}))}; Python libraries "
                             f"{sorted(gb.get('Python_Libraries', {}))}")
                lines.append(f"    Command {gb.get('Command', '').replace(str(work), '<W>')!r}")
                lines.append(f"    first line {p.read_text(encoding='utf-8').splitlines()[0]!r}")
            else:
                lines.append(f"    {json.dumps(obj, sort_keys=True)}")
        elif name.endswith(".sh"):
            lines.append(f"    {p.read_text(encoding='utf-8').replace(str(work), '<W>')!r}")
    return lines


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    work = Path(argv[0]).resolve()
    program = argv[1:]
    if work.exists():
        shutil.rmtree(work)
    inputs = work / "inputs"
    make_inputs(inputs)
    env = environment()
    for name, args, runs in CASES:
        case = work / name
        case.mkdir()
        full = [a.replace("<I>", str(inputs)) for a in args]
        if name not in NO_OUT_DIR:
            full = [*full, "--out-dir", "out"]
        print(f"== {name}: {' '.join(args)}")
        for run in range(runs):
            proc = subprocess.run(
                [*program, *full], cwd=case, env=env, capture_output=True, text=True,
                check=False, timeout=900,
            )
            print(f"-- run {run + 1}: exit {proc.returncode}")
            for line in normalise(proc.stdout, work):
                print(f"  out| {line}")
            for line in normalise(proc.stderr, work):
                print(f"  err| {line}")
        for line in describe_dir(case / "out", work):
            print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
