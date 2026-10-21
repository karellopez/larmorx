# SPDX-License-Identifier: Apache-2.0
"""Brain images in figures: slices, masks, difference maps, overlays.

Only brain slices are shown, never anything that could show a face: every anatomical slice is
masked to a (slightly dilated) brain mask, and its background is transparent so the brain sits
on the page's paper. Slices are drawn in the neurological convention (the subject's left on the
left), with their voxel sizes. The raster of each panel is embedded in the SVG at the image's
own resolution (a PNG inside the SVG), so a figure stays small and its text stays text-sharp.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

from larmorx_validation.figures import style
from larmorx_validation.figures.canvas import Canvas, wrap


@dataclass
class Volume:
    data: np.ndarray  # float64, RAS+ axes
    zooms: tuple[float, float, float]
    affine: np.ndarray


def load(path: Path, *, volume: int | None = None) -> Volume:
    """A NIfTI image in the closest canonical (RAS+) orientation, as float64."""
    import nibabel as nib

    img = nib.as_closest_canonical(nib.load(str(path)))
    data = np.asanyarray(img.dataobj, dtype=np.float64)
    if data.ndim == 4 and volume is not None:
        data = data[..., volume]
    zooms = tuple(float(z) for z in img.header.get_zooms()[:3])
    return Volume(data, zooms, img.affine)


def dilate(mask: np.ndarray, iterations: int = 2) -> np.ndarray:
    from scipy import ndimage

    return ndimage.binary_dilation(mask > 0, iterations=iterations)


@dataclass
class Plane:
    """A slice through a volume: axis 2 (axial), 1 (coronal) or 0 (sagittal), at ``index``."""

    axis: int
    index: int

    def take(self, vol: np.ndarray) -> np.ndarray:
        s = np.take(vol, self.index, axis=self.axis)
        return s.T  # rows = the second remaining axis, displayed bottom-up

    def spacing(self, zooms) -> tuple[float, float]:
        rest = [z for i, z in enumerate(zooms) if i != self.axis]
        return rest[0], rest[1]

    @property
    def name(self) -> str:
        return ("sagittal", "coronal", "axial")[self.axis]


def centre_planes(mask: np.ndarray, *, axial_shift: float = 0.1) -> tuple[Plane, Plane]:
    """An axial plane a little above the mask's centre and a coronal one through it."""
    idx = np.argwhere(mask > 0)
    lo, hi = idx.min(0), idx.max(0)
    c = (lo + hi) / 2
    k = round(float(c[2] + axial_shift * (hi[2] - lo[2])))
    return Plane(2, k), Plane(1, round(float(c[1])))


def bbox(mask2d: np.ndarray, margin: int = 3) -> tuple[slice, slice]:
    idx = np.argwhere(mask2d)
    if idx.size == 0:
        return slice(None), slice(None)
    lo = np.maximum(idx.min(0) - margin, 0)
    hi = np.minimum(idx.max(0) + margin + 1, mask2d.shape)
    return slice(lo[0], hi[0]), slice(lo[1], hi[1])


@dataclass
class Panel:
    """One image of a grid: an RGBA raster (or a scalar slice with its colour map)."""

    title: str
    image: np.ndarray  # 2-D scalar slice (rows bottom-up)
    mask: np.ndarray | None = None  # 2-D bool: transparent outside
    cmap: object = "gray"
    vmin: float | None = None
    vmax: float | None = None
    spacing: tuple[float, float] = (1.0, 1.0)  # (column, row) in mm
    contour: np.ndarray | None = None  # 2-D image whose level set is drawn
    contour_level: float | None = None
    outline: np.ndarray | None = None  # 2-D bool: its edge is drawn as a hairline
    note: str = ""  # a small label in the corner (e.g. "bit-identical")
    subtitle: str = ""
    crop: tuple[slice, slice] | None = None
    colorbar: str | None = None  # the key of a shared colour bar (see Grid.bars)


@dataclass
class Bar:
    cmap: object
    vmin: float
    vmax: float
    label: str
    ticks: list[float] | None = None
    fmt: str = "{:g}"


@dataclass
class Grid:
    panels: list[Panel]
    cols: int = 3
    cols_narrow: int = 2
    bars: dict[str, Bar] = field(default_factory=dict)
    caption_rows: list[str] = field(default_factory=list)  # a label at the left of each row
    gap: float = 14


def rgba(p: Panel, theme: style.Theme) -> np.ndarray:
    import matplotlib as mpl

    img = p.image
    cmap = mpl.colormaps[p.cmap] if isinstance(p.cmap, str) else p.cmap
    vmin = np.nanmin(img) if p.vmin is None else p.vmin
    vmax = np.nanmax(img) if p.vmax is None else p.vmax
    norm = (img - vmin) / (vmax - vmin if vmax > vmin else 1.0)
    out = cmap(np.clip(norm, 0, 1))
    if p.mask is not None:
        out[..., 3] = np.where(p.mask, 1.0, 0.0)
    out[~np.isfinite(img)] = 0
    return out


def draw_grid(theme: style.Theme, width: int, grid: Grid) -> object:
    narrow = width < 500
    cols = grid.cols_narrow if narrow else grid.cols
    gap = grid.gap
    panels = grid.panels
    # every panel the same width; heights from their aspect ratio (cropped, in mm)
    cell_w = (width - gap * (cols - 1)) / cols
    sizes = []
    for p in panels:
        img = p.image if p.crop is None else p.image[p.crop]
        h_mm = img.shape[0] * p.spacing[1]
        w_mm = img.shape[1] * p.spacing[0]
        sizes.append(cell_w * h_mm / w_mm)
    rows = [panels[i : i + cols] for i in range(0, len(panels), cols)]
    heads = [36 if any(p.subtitle for p in row) else 22 for row in rows]
    row_heights = [max(sizes[i * cols : i * cols + cols]) for i in range(len(rows))]
    bars = list(grid.bars.items())
    per_line = 1 if narrow else 2
    bw = width if per_line == 1 else (width - 3 * gap) / 2
    # a bar's label wraps to its width: 50 px for one line, 14 more for each other
    bar_h = [50 + 14 * (len(wrap(b.label, bw, style.SANS, 11)) - 1) for _, b in bars]
    bar_rows = [max(bar_h[i : i + per_line]) for i in range(0, len(bars), per_line)]
    height = sum(row_heights) + sum(heads) + len(rows) * gap + sum(bar_rows) + 4
    c = Canvas(theme, width, height)
    y = 0.0
    k = 0
    for r, row in enumerate(rows):
        for j, p in enumerate(row):
            x = j * (cell_w + gap)
            c.text(x, y + 14, p.title, size=12, color=theme.ink, weight=500)
            if p.subtitle:
                c.text(x, y + 29, p.subtitle, family=style.MONO, size=10, color=theme.ink3)
            ax = c.axes(x, y + heads[r], cell_w, sizes[k])
            _show(ax, p, theme)
            k += 1
        y += row_heights[r] + heads[r] + gap
    for i, (_key, bar) in enumerate(bars):
        bx = 0 if per_line == 1 else (i % 2) * (bw + 3 * gap)
        by = y + sum(bar_rows[: i // per_line])
        colorbar(c, bx, by + 6, bw, bar, theme)
    return c.fig


def _show(ax, p: Panel, theme: style.Theme) -> None:
    img = rgba(p, theme)
    crop = p.crop or (slice(None), slice(None))
    img = img[crop]
    h, w = img.shape[:2]
    extent = (0, w * p.spacing[0], 0, h * p.spacing[1])
    ax.imshow(img, origin="lower", extent=extent, interpolation="none", aspect="equal")
    if p.contour is not None and p.contour_level is not None:
        cimg = p.contour[crop]
        ys = (np.arange(h) + 0.5) * p.spacing[1]
        xs = (np.arange(w) + 0.5) * p.spacing[0]
        ax.contour(xs, ys, cimg, levels=[p.contour_level], colors=[theme.larmorx], linewidths=0.9)
    if p.outline is not None:
        oimg = p.outline[crop].astype(float)
        ys = (np.arange(h) + 0.5) * p.spacing[1]
        xs = (np.arange(w) + 0.5) * p.spacing[0]
        ax.contour(xs, ys, oimg, levels=[0.5], colors=[theme.ink3], linewidths=0.6)
    # a difference map that is zero everywhere says so
    note = p.note or ("bit-identical" if p.subtitle.startswith("identical") else "")
    if note:
        ax.text(
            0.5,
            0.5,
            note,
            transform=ax.transAxes,
            ha="center",
            va="center",
            fontfamily=style.MONO,
            fontsize=style.pt(10.5),
            color=theme.ink2,
            bbox={"boxstyle": "round,pad=0.4", "fc": theme.paper, "ec": theme.line, "lw": 0.6},
        )
    ax.set_xlim(extent[0], extent[1])
    ax.set_ylim(extent[2], extent[3])
    ax.axis("off")


def colorbar(c: Canvas, x: float, y: float, w: float, bar: Bar, theme: style.Theme) -> None:
    """A thin horizontal colour bar with its range stated at both ends (and 0 between, for a
    symmetric scale)."""
    import matplotlib as mpl

    cmap = mpl.colormaps[bar.cmap] if isinstance(bar.cmap, str) else bar.cmap
    lines = wrap(bar.label, w, style.SANS, 11)
    for k, line in enumerate(lines):
        c.text(x, y + 11 + 14 * k, line, size=11, color=theme.ink2)
    y += 14 * (len(lines) - 1)
    ax = c.axes(x, y + 16, w, 7)
    grad = np.linspace(0, 1, 256)[None, :]
    ax.imshow(grad, aspect="auto", cmap=cmap, extent=(bar.vmin, bar.vmax, 0, 1))
    ax.axis("off")
    ticks = bar.ticks
    zero = "0" if ticks is None else None  # a symmetric scale's middle reads "0"
    if ticks is None:
        ticks = [bar.vmin, 0.0, bar.vmax] if bar.vmin < 0 < bar.vmax else [bar.vmin, bar.vmax]
    for t in ticks:
        frac = (t - bar.vmin) / (bar.vmax - bar.vmin)
        ha = "left" if frac <= 0.001 else "right" if frac >= 0.999 else "center"
        c.text(
            x + frac * w,
            y + 37,
            zero if (t == 0 and zero) else bar.fmt.format(t),
            family=style.MONO,
            size=10,
            color=theme.ink2,
            ha=ha,
        )


def largest(diff: np.ndarray, mask: np.ndarray | None = None) -> str:
    """``largest |Δ| x`` over ``mask`` (the whole brain, not only the slice shown)."""
    v = np.abs(diff[mask] if mask is not None else diff)
    m = float(v.max()) if v.size else 0.0
    return "identical" if m == 0 else f"largest |Δ| {m:.2g}"


def symmetric(*arrays: np.ndarray, mask: np.ndarray | None = None, q: float = 99.9) -> float:
    """A symmetric range for difference maps: the ``q``-th percentile of the largest
    absolute difference among ``arrays`` (inside ``mask``), never zero."""
    vals = []
    for a in arrays:
        v = np.abs(a[mask] if mask is not None else a)
        v = v[np.isfinite(v)]
        if v.size and v.max() > 0:
            vals.append(np.percentile(v, q))
    m = max(vals) if vals else 0.0
    return float(m) if m > 0 else 1.0
