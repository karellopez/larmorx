# SPDX-License-Identifier: Apache-2.0
"""tedana 26.0.3's ``t2smap`` on ANALYZE 7.5 and MGH/MGZ inputs (relay RS2 of the clean-room
part, 2026-11-12): the replica reads them through the main package's reader of nibabel 5.4.2's
loading (``larmorx_tedana::formats``), so the suite ``tedana-t2smap-replica`` runs these cases as
it runs every other (``formats/<name>``: both environments, ``reproduce`` bit for bit, the default
pass); this module writes their inputs and names them.

What the inputs cover: SPM2's ANALYZE in both byte orders, every stored type a fit reads
(uint8, int16, int32, float32, float64), SPM2's scale factor and intercept, the gl/cal ranges'
scaling, the origin set and unset, a MATLAB ``.mat`` beside the pair (``mat`` in MATLAB 5's form,
``M`` in version 4's, neither: nibabel's error), compressed pairs, voxel sizes of 0 and below 0
(nibabel's header fixes), the image named by its ``.img``, a z-concatenated file, and a mask of
the format; MGH and MGZ of every stored type (uint8, int16, uint16, int32, float32), an oblique
affine, the repetition time of the footer and no footer, ``goodRASFlag`` 0 (nibabel's default
affine), a z-concatenated file, echoes of three formats at once; and the failures nibabel raises
(a short data file, as the first echo and as a later one; an unknown ANALYZE data type; MGH
dimensions of 0; an MGH version other than 1; a ``.mgh.gz`` name).

    from check_formats import make, CASES   # make(<folder>); CASES: (name, tier, arguments)

``<F>`` in the arguments stands for the folder ``make`` wrote.
"""

from __future__ import annotations

import gzip
from pathlib import Path

import nibabel as nb
import numpy as np
from nibabel.spm2analyze import Spm2AnalyzeHeader

SHAPE = (6, 7, 4, 5)
TES = (0.014, 0.030, 0.046)


def _echoes() -> list[np.ndarray]:
    rng = np.random.default_rng(20261112)
    s0 = 900 + 300 * rng.random(SHAPE[:3])
    t2 = 0.02 + 0.04 * rng.random(SHAPE[:3])
    out = []
    for te in TES:
        sig = (s0 * np.exp(-te / t2))[..., None] * (1 + 0.02 * rng.standard_normal(SHAPE))
        sig[0, 0, :, :] = rng.standard_normal(SHAPE[2:]) * 3  # a few voxels of noise
        out.append(sig)
    return out


def _head_echoes() -> list[np.ndarray]:
    """A head-shaped series nilearn finds a mask in (16 x 14 x 8 voxels, 5 volumes): an
    ellipsoid of S0 900-1200 decaying in each echo, a background of about 20."""
    rng = np.random.default_rng(20261113)
    shape = (16, 14, 8, 5)
    i, j, k = np.indices(shape[:3])
    head = ((i - 7.5) / 7) ** 2 + ((j - 6.5) / 6) ** 2 + ((k - 3.5) / 3.5) ** 2 <= 1
    s0 = np.where(head, 900 + 300 * rng.random(shape[:3]), 20.0)
    t2 = 0.02 + 0.04 * rng.random(shape[:3])
    out = []
    for te in TES:
        decay = np.where(head, np.exp(-te / t2), 1.0)
        out.append((s0 * decay)[..., None] * (1 + 0.02 * rng.standard_normal(shape)))
    return out


def write_analyze(
    path: Path,
    data: np.ndarray,
    *,
    dtype: str,
    endian: str = "<",
    pixdim=(3.0, 3.0, 3.5),
    origin=(0, 0, 0),
    slope: float = 1.0,
    inter: float = 0.0,
    gl: tuple[int, int] | None = None,
    cal: tuple[float, float] | None = None,
    code: int | None = None,
    gz: bool = False,
) -> None:
    """A ``.hdr``/``.img`` pair of SPM2's ANALYZE, written field by field (``path`` the
    ``.hdr``)."""
    hdr = Spm2AnalyzeHeader(endianness=endian)
    hdr.set_data_dtype(np.dtype(dtype))
    hdr.set_data_shape(data.shape)
    pd = hdr["pixdim"]
    pd[1 : 1 + len(pixdim)] = pixdim
    if data.ndim == 4:
        pd[4] = 2.5
    hdr["pixdim"] = pd
    hdr["origin"][:3] = origin
    hdr["scl_slope"] = slope
    hdr["scl_inter"] = inter
    if gl is not None:
        hdr["glmax"], hdr["glmin"] = gl
    if cal is not None:
        hdr["cal_max"], hdr["cal_min"] = cal
    if code is not None:
        hdr["datatype"] = code
    raw = np.asarray(data).astype(np.dtype(dtype).newbyteorder(endian)).tobytes(order="F")
    opener = gzip.open if gz else open
    with opener(str(path) + (".gz" if gz else ""), "wb") as f:
        f.write(hdr.binaryblock)
    with opener(str(path.with_suffix(".img")) + (".gz" if gz else ""), "wb") as f:
        f.write(raw)


def write_mat(path: Path, key: str, m: np.ndarray, version: str) -> None:
    import scipy.io as sio

    sio.savemat(str(path), {key: m}, format=version)


def write_mgh(
    path: Path,
    data: np.ndarray,
    affine: np.ndarray,
    *,
    tr: float | None = 2500.0,
    good_ras: bool = True,
    patch: dict[int, bytes] | None = None,
    cut: int = 0,
) -> None:
    img = nb.MGHImage(data, affine)
    if tr is not None:
        img.header["tr"] = tr
    tmp = path.with_name(path.name + ".tmp.mgh")
    img.to_filename(tmp)
    b = bytearray(tmp.read_bytes())
    tmp.unlink()
    if not good_ras:
        b[28:30] = b"\x00\x00"
    for off, val in (patch or {}).items():
        b[off : off + len(val)] = val
    if tr is None:
        n = int(np.prod(data.shape)) * data.dtype.itemsize
        b = b[: 284 + n]
    if cut:
        b = b[:-cut]
    if path.suffix.lower() == ".mgz":
        path.write_bytes(gzip.compress(bytes(b), mtime=0))
    else:
        path.write_bytes(bytes(b))


def make(d: Path) -> None:
    d.mkdir(parents=True, exist_ok=True)
    ech = _echoes()
    mask = np.zeros(SHAPE[:3], np.uint8)
    mask[1:5, 1:6, :] = 1
    aff = np.diag([3.0, 3.0, 3.5, 1.0])
    nb.Nifti1Image(mask, aff).to_filename(d / "mask.nii.gz")
    for i, x in enumerate(ech, 1):
        write_analyze(d / f"a32-e{i}.hdr", x, dtype="f4", origin=(3, 4, 2))
        # int16 big-endian, SPM2's scale factor and intercept: stored = (x - 0.5) / 2
        write_analyze(
            d / f"a16be-e{i}.hdr",
            np.round((x - 0.5) / 2),
            dtype="i2",
            endian=">",
            slope=2.0,
            inter=0.5,
        )
        write_mat(
            d / f"a16be-e{i}.mat",
            "mat",
            np.array([[-3.0, 0, 0, 9], [0, 3, 0, -12], [0, 0, 3.5, -7], [0, 0, 0, 1]]),
            "5",
        )
        # uint8 with no scale factor: the gl/cal ranges' slope (0..255 -> 0..1530)
        write_analyze(
            d / f"a8-e{i}.hdr",
            np.clip(np.round(x / 6), 0, 255),
            dtype="u1",
            slope=0.0,
            gl=(255, 0),
            cal=(1530.0, 0.0),
        )
        write_mat(
            d / f"a8-e{i}.mat",
            "M",
            np.array([[3.0, 0, 0, -9], [0, 3, 0, -12], [0, 0, 3.5, -7], [0, 0, 0, 1]]),
            "4",
        )
        # compressed float64 pairs with a voxel size of 0 and one below 0
        write_analyze(d / f"a64-e{i}.hdr", x, dtype="f8", pixdim=(0.0, -3.0, 3.5), gz=True)
        # MGH: float32 with an oblique affine and the footer's repetition time
        c, s = np.cos(0.3), np.sin(0.3)
        obl = np.array(
            [[3 * c, -3 * s, 0, 10.5], [3 * s, 3 * c, 0, -20.25], [0, 0, 3.5, 5.125], [0, 0, 0, 1]]
        )
        write_mgh(d / f"m32-e{i}.mgh", x.astype(np.float32), obl)
        write_mgh(d / f"m16-e{i}.mgz", np.round(x).astype(np.int16), aff)
    # computed masks (nilearn's compute_epi_mask of the first echo) need a head: int16 MGZ, and
    # big-endian int16 ANALYZE with SPM2's scale factor and a MATLAB 5 `mat`
    for i, x in enumerate(_head_echoes(), 1):
        write_mgh(d / f"mhead-e{i}.mgz", np.round(x).astype(np.int16), aff)
        write_analyze(
            d / f"ahead-e{i}.hdr",
            np.round((x - 0.5) / 2),
            dtype="i2",
            endian=">",
            slope=2.0,
            inter=0.5,
        )
        write_mat(
            d / f"ahead-e{i}.mat",
            "mat",
            np.array([[-3.0, 0, 0, 9], [0, 3, 0, -12], [0, 0, 3.5, -7], [0, 0, 0, 1]]),
            "5",
        )
        write_mgh(d / f"mu16-e{i}.mgz", np.round(np.abs(x)).astype(np.uint16), aff, good_ras=False)
        write_mgh(d / f"m8-e{i}.mgz", np.clip(np.round(x / 6), 0, 255).astype(np.uint8), aff)
    # a mask of each format
    write_analyze(d / "amask.hdr", mask, dtype="u1", origin=(3, 4, 2))
    write_analyze(d / "amask64.hdr", mask, dtype="u1", pixdim=(0.0, -3.0, 3.5), gz=True)
    write_mgh(
        d / "mmask.mgz",
        mask,
        np.array(
            [[3 * c, -3 * s, 0, 10.5], [3 * s, 3 * c, 0, -20.25], [0, 0, 3.5, 5.125], [0, 0, 0, 1]]
        ),
        tr=None,
    )
    # z-concatenated: int32 ANALYZE and int32 MGH without a footer
    zcat = np.concatenate([np.round(x) for x in ech], axis=2)
    write_analyze(d / "azcat.hdr", zcat, dtype="i4")
    write_mgh(d / "mzcat.mgh", zcat.astype(np.int32), aff, tr=None)
    # failures
    write_analyze(d / "short-e1.hdr", ech[0], dtype="f4")
    img = (d / "short-e1.img").read_bytes()
    (d / "short-e1.img").write_bytes(img[: len(img) - 40])
    write_analyze(d / "short-e2.hdr", ech[1], dtype="f4")
    img = (d / "short-e2.img").read_bytes()
    (d / "short-e2.img").write_bytes(img[: len(img) - 40])
    write_analyze(d / "code7-e1.hdr", ech[0], dtype="f4", code=7)
    write_mat(d / "nokey-e1.mat", "other", np.eye(4), "5")
    write_analyze(d / "nokey-e1.hdr", ech[0], dtype="f4")
    write_mgh(d / "dims0-e1.mgh", ech[0].astype(np.float32), aff, patch={4: b"\x00\x00\x00\x00"})
    write_mgh(d / "version2-e1.mgh", ech[0].astype(np.float32), aff, patch={0: b"\x00\x00\x00\x02"})
    write_mgh(d / "mshort-e2.mgh", ech[1].astype(np.float32), aff, tr=None, cut=40)
    (d / "gzname-e1.mgh.gz").write_bytes(gzip.compress((d / "m32-e1.mgh").read_bytes(), mtime=0))


def _d(stem: str, ext: str) -> list[str]:
    return ["-d", *(f"<F>/{stem}-e{i}{ext}" for i in (1, 2, 3))]


TE = ["-e", *(str(t) for t in TES)]
M = ["--mask", "<F>/mask.nii.gz"]
CASES: list[tuple[str, str, list[str]]] = [
    ("analyze-f32", "smoke", [*_d("a32", ".hdr"), *TE, "--mask", "<F>/amask.hdr"]),
    (
        "analyze-i16-be-scaled-mat",
        "standard",
        [
            *_d("a16be", ".hdr"),
            *TE,
            "--mask",
            "<F>/amask.hdr",
            "--fittype",
            "curvefit",
            "--verbose",
        ],
    ),
    ("analyze-head-nomask", "standard", [*_d("ahead", ".hdr"), *TE]),
    ("analyze-u8-glcal-M", "standard", [*_d("a8", ".img"), *TE, *M, "--combmode", "paid"]),
    (
        "analyze-gz-f64-pixdim",
        "standard",
        [*_d("a64", ".hdr.gz"), *TE, "--mask", "<F>/amask64.hdr.gz", "--fitmode", "ts"],
    ),
    ("analyze-zcat-i32", "standard", ["-d", "<F>/azcat.hdr", *TE, *M]),
    ("mgh-f32-oblique", "smoke", [*_d("m32", ".mgh"), *TE, "--mask", "<F>/mmask.mgz"]),
    (
        "mgz-i16-nomask-curvefit",
        "standard",
        [*_d("mhead", ".mgz"), *TE, "--fittype", "curvefit", "--masktype", "dropout", "decay"],
    ),
    ("mgz-u16-noras", "standard", [*_d("mu16", ".mgz"), *TE, *M]),
    ("mgz-u8", "standard", [*_d("m8", ".mgz"), *TE, *M, "--dummy-scans", "1"]),
    ("mgh-zcat-i32-nofooter", "standard", ["-d", "<F>/mzcat.mgh", *TE, *M]),
    (
        "mixed-formats",
        "standard",
        ["-d", "<F>/a32-e1.hdr", "<F>/m32-e2.mgh", "<F>/m16-e3.mgz", *TE, *M],
    ),
    (
        "x-short-first",
        "standard",
        ["-d", "<F>/short-e1.hdr", "<F>/a32-e2.hdr", "<F>/a32-e3.hdr", *TE, *M],
    ),
    (
        "x-short-later",
        "standard",
        ["-d", "<F>/a32-e1.hdr", "<F>/short-e2.hdr", "<F>/a32-e3.hdr", *TE, *M],
    ),
    (
        "x-analyze-code7",
        "standard",
        ["-d", "<F>/code7-e1.hdr", "<F>/a32-e2.hdr", "<F>/a32-e3.hdr", *TE, *M],
    ),
    (
        "x-mat-no-key",
        "standard",
        ["-d", "<F>/nokey-e1.hdr", "<F>/a32-e2.hdr", "<F>/a32-e3.hdr", *TE, *M],
    ),
    (
        "x-mgh-dims0",
        "standard",
        ["-d", "<F>/dims0-e1.mgh", "<F>/m32-e2.mgh", "<F>/m32-e3.mgh", *TE, *M],
    ),
    (
        "x-mgh-version",
        "standard",
        ["-d", "<F>/version2-e1.mgh", "<F>/m32-e2.mgh", "<F>/m32-e3.mgh", *TE, *M],
    ),
    (
        "x-mgh-short-later",
        "standard",
        ["-d", "<F>/m32-e1.mgh", "<F>/mshort-e2.mgh", "<F>/m32-e3.mgh", *TE, *M],
    ),
    (
        "x-mgh-gz-name",
        "standard",
        ["-d", "<F>/gzname-e1.mgh.gz", "<F>/m32-e2.mgh", "<F>/m32-e3.mgh", *TE, *M],
    ),
]


def arguments(args: list[str], folder: Path) -> list[str]:
    return [a.replace("<F>", str(folder)) for a in args]
