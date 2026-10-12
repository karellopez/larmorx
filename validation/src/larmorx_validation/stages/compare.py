# SPDX-License-Identifier: Apache-2.0
"""How far one image, series or transform set is from another.

Images are read with nibabel, a reader independent of larmorx and of the tools compared.
Intensity images get bit-identity, the largest absolute and relative difference and the
normalized cross-correlation (NCC); masks and label maps get Dice and the number of changed
voxels; 4D series are compared volume by volume, so a large series never needs two float64
copies in memory.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

import numpy as np

#: A map with at most this many distinct integer values is treated as labels.
MAX_LABELS = 256


@dataclass
class ImageDiff:
    """The difference between a reference image and a test image."""

    shape: tuple[int, ...]
    dtype_ref: str
    dtype_test: str
    voxels: int
    #: Values whose bits differ (as stored when both have the same type, else as values).
    differing: int
    max_abs: float
    #: ``max |diff| / max |reference|``.
    max_rel: float
    #: Largest per-voxel ``|diff| / |reference|`` over voxels where the reference is not 0.
    max_rel_voxel: float
    ncc: float
    #: The largest difference between the two affines (mm).
    affine_diff: float
    #: ``"labels"`` (masks and label maps) or ``"intensity"``.
    kind: str
    dice: float | None = None
    #: Per label: Dice; only for label maps with more than one label.
    dice_labels: dict[str, float] = field(default_factory=dict)
    changed: int | None = None
    #: 4D only: the PLAN.md §11.3 "preprocessed BOLD" measures.
    bold: dict[str, float] | None = None
    #: 3D intensity images: the voxels whose ``|diff| / max |reference|`` exceeds each of
    #: PLAN.md §11.3's relative thresholds (keys "1e-06", "1e-05", "0.0001").
    over: dict[str, int] = field(default_factory=dict)
    note: str = ""

    @property
    def identical(self) -> bool:
        return self.differing == 0 and self.shape != ()

    def to_dict(self) -> dict[str, Any]:
        d = asdict(self)
        d["shape"] = list(self.shape)
        return {k: v for k, v in d.items() if v is not None and v != {} and v != ""}

    def summary(self) -> str:
        """One line for the report."""
        if self.differing == 0:
            return "bit-identical"
        if self.kind == "labels":
            text = f"{self.changed:,} of {self.voxels:,} voxels changed, Dice {self.dice:.6f}"
        else:
            text = (
                f"{self.differing:,} of {self.voxels:,} differ; max |diff| {self.max_abs:.3g} "
                f"(rel {self.max_rel:.2g}), NCC {_ncc_text(self.ncc)}"
            )
        return text


def _ncc_text(ncc: float) -> str:
    if ncc >= 1.0:
        return "1"
    gap = 1.0 - ncc
    return f"1 - {gap:.2g}" if gap < 1e-3 else f"{ncc:.6f}"


def load(path: str | Path):
    """``(values, affine, stored dtype)``: the values as stored (no scaling applied unless the
    file is scaled), read with nibabel."""
    import nibabel as nib

    img = nib.load(str(path))
    proxy = img.dataobj
    slope, inter = getattr(proxy, "slope", 1.0), getattr(proxy, "inter", 0.0)
    if (slope in (1.0, None) or np.isnan(slope)) and (inter in (0.0, None) or np.isnan(inter)):
        data = np.asanyarray(proxy.get_unscaled() if hasattr(proxy, "get_unscaled") else proxy)
    else:
        data = np.asanyarray(proxy)
    return data, img.affine, img.get_data_dtype()


def _bits(a: np.ndarray) -> np.ndarray:
    a = np.ascontiguousarray(a)
    if a.dtype.kind == "f":
        a = np.where(np.isnan(a), np.array(np.nan, dtype=a.dtype), a)
        return a.view(f"u{a.dtype.itemsize}")
    return a


def _is_labels(a: np.ndarray, b: np.ndarray) -> bool:
    for x in (a, b):
        if x.dtype.kind == "f":
            sample = x.ravel()[:: max(1, x.size // 2_000_000)]
            if not np.all(np.isfinite(sample)) or not np.array_equal(sample, np.round(sample)):
                return False
    ua = np.unique(a.ravel()[:: max(1, a.size // 4_000_000)])
    return len(ua) <= MAX_LABELS and len(np.unique(b)) <= MAX_LABELS


class _Accumulator:
    """Running sums for the difference statistics of arrays seen in pieces."""

    def __init__(self) -> None:
        self.n = 0
        self.differing = 0
        self.max_abs = 0.0
        self.max_ref = 0.0
        self.max_rel_voxel = 0.0
        self.sa = self.sb = self.saa = self.sbb = self.sab = 0.0

    def add(self, a: np.ndarray, b: np.ndarray, same_type: bool) -> None:
        if same_type:
            self.differing += int(np.count_nonzero(_bits(a) != _bits(b)))
        else:
            self.differing += int(np.count_nonzero(a.astype(np.float64) != b.astype(np.float64)))
        af = a.astype(np.float64).ravel()
        bf = b.astype(np.float64).ravel()
        ok = np.isfinite(af) & np.isfinite(bf)
        af, bf = af[ok], bf[ok]
        self.n += af.size
        if af.size:
            d = np.abs(af - bf)
            self.max_abs = max(self.max_abs, float(d.max()))
            self.max_ref = max(self.max_ref, float(np.abs(af).max()))
            nz = af != 0
            if nz.any():
                self.max_rel_voxel = max(self.max_rel_voxel, float((d[nz] / np.abs(af[nz])).max()))
            self.sa += float(af.sum())
            self.sb += float(bf.sum())
            self.saa += float(af @ af)
            self.sbb += float(bf @ bf)
            self.sab += float(af @ bf)

    def ncc(self) -> float:
        if self.n == 0:
            return 1.0
        n = self.n
        cov = self.sab - self.sa * self.sb / n
        va = self.saa - self.sa * self.sa / n
        vb = self.sbb - self.sb * self.sb / n
        if va <= 0 or vb <= 0:
            return 1.0 if self.differing == 0 else 0.0
        return float(min(1.0, cov / np.sqrt(va * vb)))


def _dice(a: np.ndarray, b: np.ndarray) -> float:
    sa, sb = int(np.count_nonzero(a)), int(np.count_nonzero(b))
    if sa + sb == 0:
        return 1.0
    return 2.0 * int(np.count_nonzero(a & b)) / (sa + sb)


def compare_arrays(
    ref: np.ndarray, test: np.ndarray, *, affine_diff: float = 0.0, kind: str | None = None
) -> ImageDiff:
    """Compare two arrays of the same shape (3D or 4D)."""
    if ref.shape != test.shape:
        return ImageDiff(
            shape=tuple(ref.shape),
            dtype_ref=str(ref.dtype),
            dtype_test=str(test.dtype),
            voxels=int(ref.size),
            differing=int(ref.size),
            max_abs=float("inf"),
            max_rel=float("inf"),
            max_rel_voxel=float("inf"),
            ncc=0.0,
            affine_diff=affine_diff,
            kind=kind or "intensity",
            note=f"shapes differ: reference {ref.shape}, test {test.shape}",
        )
    same_type = ref.dtype == test.dtype
    acc = _Accumulator()
    if ref.ndim == 4:
        for t in range(ref.shape[3]):
            acc.add(np.asarray(ref[..., t]), np.asarray(test[..., t]), same_type)
    else:
        acc.add(np.asarray(ref), np.asarray(test), same_type)
    kind = kind or ("labels" if ref.ndim == 3 and _is_labels(ref, test) else "intensity")
    diff = ImageDiff(
        shape=tuple(ref.shape),
        dtype_ref=str(ref.dtype),
        dtype_test=str(test.dtype),
        voxels=int(ref.size),
        differing=acc.differing,
        max_abs=acc.max_abs,
        max_rel=acc.max_abs / acc.max_ref
        if acc.max_ref > 0
        else (0.0 if acc.max_abs == 0 else 1.0),
        max_rel_voxel=acc.max_rel_voxel,
        ncc=acc.ncc(),
        affine_diff=affine_diff,
        kind=kind,
    )
    if kind == "intensity" and ref.ndim == 3 and diff.differing and acc.max_ref > 0:
        d = np.abs(np.asarray(ref, dtype=np.float64) - np.asarray(test, dtype=np.float64))
        d = np.where(np.isnan(d), 0.0, d) / acc.max_ref
        diff.over = {f"{t:g}": int(np.count_nonzero(d > t)) for t in (1e-6, 1e-5, 1e-4)}
    if kind == "labels":
        a, b = np.asarray(ref), np.asarray(test)
        diff.changed = int(np.count_nonzero(a != b))
        diff.dice = _dice(a != 0, b != 0)
        labels = [v for v in np.union1d(np.unique(a), np.unique(b)) if v != 0]
        if len(labels) > 1:
            diff.dice_labels = {
                str(int(v) if float(v).is_integer() else float(v)): _dice(a == v, b == v)
                for v in labels
            }
    if ref.ndim == 4 and diff.differing:
        diff.bold = bold_measures(ref, test)
    return diff


def compare_files(
    ref_path: str | Path, test_path: str | Path, kind: str | None = None
) -> ImageDiff:
    """Compare two image files."""
    a, aa, _ = load(ref_path)
    b, ab, _ = load(test_path)
    return compare_arrays(a, b, affine_diff=float(np.abs(aa - ab).max()), kind=kind)


def raw_header(path: str | Path):
    """A NIfTI file's header as stored (``nib.load`` moves ``scl_slope`` and ``scl_inter`` to
    the data proxy and resets them in the header it returns), with its extensions."""
    import nibabel as nib
    from nibabel.openers import Opener

    kind = type(nib.load(str(path)).header)
    with Opener(str(path)) as f:
        return kind.from_fileobj(f)


def stored_diff(ref_path: str | Path, test_path: str | Path) -> dict[str, Any]:
    """Two files' data as stored, before any scaling: the stored type, the scale factors and,
    for integer types, the largest difference in stored units."""
    import nibabel as nib

    ra, rb = nib.load(str(ref_path)), nib.load(str(test_path))
    ha, hb = raw_header(ref_path), raw_header(test_path)
    out: dict[str, Any] = {
        "dtype": [str(ra.get_data_dtype()), str(rb.get_data_dtype())],
        "scl_slope": [float(ha["scl_slope"]), float(hb["scl_slope"])],
        "scl_inter": [float(ha["scl_inter"]), float(hb["scl_inter"])],
    }
    same = ra.get_data_dtype() == rb.get_data_dtype()
    if same and ra.get_data_dtype().kind in "iu" and ra.shape == rb.shape:
        a = np.asanyarray(ra.dataobj.get_unscaled())
        b = np.asanyarray(rb.dataobj.get_unscaled())
        d = np.abs(a.astype(np.int64) - b.astype(np.int64))
        out["differing"] = int(np.count_nonzero(d))
        out["max_abs"] = int(d.max())
        out["values"] = int(d.size)
    return out


def header_diff(ref_path: str | Path, test_path: str | Path) -> dict[str, Any]:
    """The NIfTI header fields (as nibabel reads them) in which two files differ, and whether
    their header extensions have the same codes and contents."""
    a = raw_header(ref_path)
    b = raw_header(test_path)
    fields = []
    if type(a) is not type(b):
        fields.append("format")
    else:
        for key in a.keys():  # noqa: SIM118 (nibabel headers are not plain mappings)
            va, vb = np.asarray(a[key]), np.asarray(b[key])
            if va.shape != vb.shape or va.tobytes() != vb.tobytes():
                fields.append(key)
    ea = [(int(e.get_code()), e.get_content()) for e in getattr(a, "extensions", [])]
    eb = [(int(e.get_code()), e.get_content()) for e in getattr(b, "extensions", [])]
    return {
        "identical": not fields,
        "fields": fields,
        "extension_codes": [[c for c, _ in ea], [c for c, _ in eb]],
        "extensions_identical": ea == eb,
    }


def bold_measures(ref: np.ndarray, test: np.ndarray) -> dict[str, float]:
    """PLAN.md §11.3's measures for preprocessed BOLD, inside the voxels where the reference's
    temporal mean is above 10 % of its maximum: NCC of the temporal means, correlation of the
    tSNR maps, median per-voxel time-series correlation."""
    a = np.asarray(ref, dtype=np.float32)
    b = np.asarray(test, dtype=np.float32)
    ma, mb = a.mean(axis=3, dtype=np.float64), b.mean(axis=3, dtype=np.float64)
    mask = ma > 0.1 * ma.max()
    if not mask.any():
        return {}
    xa, xb = a[mask].astype(np.float64), b[mask].astype(np.float64)
    sa, sb = xa.std(axis=1), xb.std(axis=1)
    ta = np.where(sa > 0, xa.mean(axis=1) / np.where(sa > 0, sa, 1), 0)
    tb = np.where(sb > 0, xb.mean(axis=1) / np.where(sb > 0, sb, 1), 0)
    ca, cb = xa - xa.mean(axis=1, keepdims=True), xb - xb.mean(axis=1, keepdims=True)
    den = np.sqrt((ca * ca).sum(axis=1) * (cb * cb).sum(axis=1))
    r = np.where(den > 0, (ca * cb).sum(axis=1) / np.where(den > 0, den, 1), 1.0)
    return {
        "mask_voxels": int(mask.sum()),
        "mean_ncc": float(np.corrcoef(ma[mask], mb[mask])[0, 1]),
        "tsnr_r": float(np.corrcoef(ta, tb)[0, 1]),
        "timeseries_r_median": float(np.median(r)),
    }


# ------------------------------------------------------------------------------------------------
# Transforms and motion


def rms_deviation(
    m1: np.ndarray, m2: np.ndarray, centre: np.ndarray, radius: float = 80.0
) -> float:
    """Jenkinson's RMS deviation (mm) between two 4×4 matrices, over a sphere of ``radius``
    about ``centre``."""
    d = m1 @ np.linalg.inv(m2) - np.eye(4)
    a = d[:3, :3]
    t = d[:3, 3] + a @ centre
    return float(np.sqrt(t @ t + radius * radius / 5.0 * np.trace(a.T @ a)))


def decompose(m: np.ndarray, centre: np.ndarray) -> np.ndarray:
    """``rx ry rz`` (radians) and ``tx ty tz`` (mm) of a rigid FSL matrix about ``centre``."""
    r = m[:3, :3]
    cy = np.hypot(r[0, 0], r[0, 1])
    if cy < 1e-4:
        angles = [np.arctan2(-r[2, 1], r[1, 1]), np.arctan2(-r[0, 2], 0.0), 0.0]
    else:
        angles = [
            np.arctan2(r[1, 2] / cy, r[2, 2] / cy),
            np.arctan2(-r[0, 2], cy),
            np.arctan2(r[0, 1] / cy, r[0, 0] / cy),
        ]
    t = r @ centre + m[:3, 3] - centre
    return np.array([*angles, *t])


def read_mats(directory: Path) -> list[np.ndarray]:
    """``MAT_0000``, ``MAT_0001``, ... of an mcflirt ``.mat`` directory."""
    files = sorted(directory.glob("MAT_*"), key=lambda p: int(p.name[4:]))
    return [
        np.array([[float(x) for x in line.split()] for line in f.read_text().splitlines()[:4]])
        for f in files
    ]


def read_itk_affines(path: Path) -> np.ndarray:
    """The ``(N, 4, 4)`` affines of an ITK text transform file (LPS, as stored)."""
    out = []
    params: list[float] | None = None
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("Parameters:"):
            params = [float(x) for x in line.split()[1:]]
        elif line.startswith("FixedParameters:") and params is not None:
            fixed = np.array([float(x) for x in line.split()[1:4]])
            m = np.eye(4)
            m[:3, :3] = np.array(params[:9]).reshape(3, 3)
            # ITK: y = A (x - c) + t + c
            m[:3, 3] = np.array(params[9:12]) + fixed - m[:3, :3] @ fixed
            out.append(m)
            params = None
    return np.array(out)


def motion_diff(ref: np.ndarray, test: np.ndarray, centre: np.ndarray) -> dict[str, float]:
    """Per-volume comparison of two sets of rigid FSL matrices: RMS deviation and the six
    parameter differences (mm, degrees)."""
    rms = np.array([rms_deviation(a, b, centre) for a, b in zip(ref, test, strict=True)])
    pa = np.array([decompose(m, centre) for m in ref])
    pb = np.array([decompose(m, centre) for m in test])
    d = np.abs(pa - pb)
    deg = np.degrees(d[:, :3])
    mm = d[:, 3:]
    return {
        "volumes": len(rms),
        "rms_median_mm": float(np.median(rms)),
        "rms_p95_mm": float(np.percentile(rms, 95)),
        "rms_max_mm": float(rms.max()),
        "median_mm": float(np.median(mm)),
        "median_deg": float(np.median(deg)),
        "max_mm": float(mm.max()),
        "max_deg": float(deg.max()),
        "identical_volumes": int(sum(np.array_equal(a, b) for a, b in zip(ref, test, strict=True))),
    }


__all__ = [
    "ImageDiff",
    "bold_measures",
    "compare_arrays",
    "compare_files",
    "decompose",
    "header_diff",
    "load",
    "motion_diff",
    "raw_header",
    "read_itk_affines",
    "read_mats",
    "rms_deviation",
    "stored_diff",
]
