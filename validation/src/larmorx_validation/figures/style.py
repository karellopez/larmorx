# SPDX-License-Identifier: Apache-2.0
"""One style for every figure: the documentation site's design language.

Warm paper, ink text, hairlines; Instrument Serif for titles and numerals, Inter for text,
JetBrains Mono for labels and ticks. Every figure is drawn twice, once for each of the site's
themes, on the colour of the site's figure card (``--bg-elev``), and its colours are chosen
against that paper.

Colours have one job each (validated with a colour-vision-deficiency check: OKLab ΔE ≥ 13
between neighbours under protanopia and deuteranopia, normal-vision ΔE ≥ 15, 3:1 against the
card, in both themes):

- **Programs** (who produced a result): the original program in ink, LARMORX in its apricot,
  an exact replica in blue. Never the ecosystem's own pastel: an ecosystem's colour stays the
  site's way of naming the ecosystem (a dot beside its name), not a series.
- **Agreement states** (the overview of a validation record): identical to the unmodified
  original in apricot; identical once the original uses correctly rounded maths in apricot
  with a hatch; within the record's thresholds in blue; an expected divergence in ochre; a
  failure in magenta; rejected by both in a hatched neutral grey.
- **Differences** (signed): blue below zero, the paper at zero, apricot above, on a symmetric
  scale whose range the figure states. Magnitudes: one hue, from the paper to apricot.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

#: Design widths, in CSS pixels: the site's text column, and a phone.
WIDE = 720
NARROW = 360

SERIF = "Instrument Serif"
SANS = "Inter"
MONO = "JetBrains Mono"


def pt(px: float) -> float:
    """A CSS pixel size in points (matplotlib's unit; an SVG is drawn at 96 px per inch)."""
    return px * 0.75


def inches(px: float) -> float:
    return px / 96.0


@dataclass(frozen=True)
class Theme:
    name: str
    paper: str  # the card the figure sits on (the site's --bg-elev)
    sunk: str  # a recessed surface (--bg-sunk)
    ink: str
    ink2: str
    ink3: str
    line: tuple[float, float, float, float]
    line_soft: tuple[float, float, float, float]
    # programs
    original: str
    larmorx: str
    replica: str
    # agreement states
    identical: str
    within: str
    expected: str
    fail: str
    both: str
    # the larmorx hue, light (a wash)
    larmorx_wash: str
    eco: dict[str, str] = field(default_factory=dict)

    @property
    def states(self) -> dict[str, dict]:
        """How each agreement state is drawn: fill colour, hatch, legend label."""
        return {
            "identical": {"color": self.identical, "hatch": None},
            "identical-cm": {"color": self.larmorx_wash, "hatch": "////", "edge": self.identical},
            "agree": {"color": self.larmorx_wash, "hatch": None},
            "within": {"color": self.within, "hatch": None},
            "expected": {"color": self.expected, "hatch": None},
            "fail": {"color": self.fail, "hatch": None},
            "both": {"color": self.sunk, "hatch": "\\\\\\\\", "edge": self.both},
        }


def _rgba(hex_: str, alpha: float) -> tuple[float, float, float, float]:
    h = hex_.lstrip("#")
    return (int(h[0:2], 16) / 255, int(h[2:4], 16) / 255, int(h[4:6], 16) / 255, alpha)


ECO_LIGHT = {
    "ants": "#467dea",
    "afni": "#987e2e",
    "fsl": "#438c84",
    "freesurfer": "#9860f6",
    "workbench": "#d8489f",
    "larmorx": "#c76717",
}
ECO_DARK = {
    "ants": "#9db9ec",
    "afni": "#e3c779",
    "fsl": "#86cfc6",
    "freesurfer": "#bba8ec",
    "workbench": "#eeaacd",
    "larmorx": "#f2b28a",
}

LIGHT = Theme(
    name="light",
    paper="#f0ebe0",
    sunk="#ddd5c4",
    ink="#16130f",
    ink2="#4a453e",
    ink3="#5b554c",
    line=_rgba("#16130f", 0.20),
    line_soft=_rgba("#16130f", 0.11),
    original="#4a453e",
    larmorx="#c76717",
    replica="#467dea",
    identical="#c76717",
    within="#467dea",
    expected="#987e2e",
    fail="#d8489f",
    both="#8a847a",
    larmorx_wash="#f2c9a8",
    eco=ECO_LIGHT,
)
DARK = Theme(
    name="dark",
    paper="#1a1916",
    sunk="#2a2824",
    ink="#efebe2",
    ink2="#b6b0a3",
    ink3="#908a7f",
    line=_rgba("#efebe2", 0.20),
    line_soft=_rgba("#efebe2", 0.11),
    original="#b6b0a3",
    larmorx="#d97530",
    replica="#5890fa",
    identical="#d97530",
    within="#5890fa",
    expected="#b08f2d",
    fail="#e754ab",
    both="#7d776d",
    larmorx_wash="#6b4123",
    eco=ECO_DARK,
)
THEMES = (LIGHT, DARK)

STATE_LABELS = {
    "identical": "identical to the unmodified original",
    "identical-cm": "identical with correctly rounded maths",
    "agree": "identical (split above)",
    "within": "within the record's thresholds",
    "expected": "expected divergence",
    "fail": "failure",
    "both": "rejected by both",
}
STATE_ORDER = ("identical", "identical-cm", "agree", "within", "expected", "fail", "both")


def rc(theme: Theme) -> dict:
    """matplotlib settings for ``theme``."""
    return {
        "font.family": SANS,
        "font.size": pt(12),
        "font.weight": 400,
        "text.color": theme.ink,
        "axes.edgecolor": theme.line,
        "axes.labelcolor": theme.ink2,
        "axes.labelsize": pt(12),
        "axes.labelweight": 400,
        "axes.linewidth": 0.75,
        "axes.facecolor": "none",
        "axes.titlesize": pt(17),
        "axes.titlecolor": theme.ink,
        "axes.spines.top": False,
        "axes.spines.right": False,
        "axes.grid": False,
        "axes.axisbelow": True,
        "grid.color": theme.line_soft,
        "grid.linewidth": 0.75,
        "grid.linestyle": "-",
        "xtick.color": theme.line,
        "ytick.color": theme.line,
        "xtick.labelcolor": theme.ink2,
        "ytick.labelcolor": theme.ink2,
        "xtick.labelsize": pt(11),
        "ytick.labelsize": pt(11),
        "xtick.major.size": 3,
        "ytick.major.size": 3,
        "xtick.minor.size": 1.5,
        "ytick.minor.size": 1.5,
        "xtick.major.width": 0.75,
        "ytick.major.width": 0.75,
        "legend.frameon": False,
        "legend.fontsize": pt(11.5),
        "legend.labelcolor": theme.ink2,
        "legend.handlelength": 1.2,
        "legend.handleheight": 0.9,
        "legend.borderaxespad": 0.2,
        "figure.facecolor": "none",
        "figure.edgecolor": "none",
        "savefig.facecolor": "none",
        "savefig.edgecolor": "none",
        "savefig.transparent": False,
        "hatch.linewidth": 0.8,
        "hatch.color": theme.both,
        "lines.linewidth": 1.5,
        "lines.solid_capstyle": "round",
        "patch.linewidth": 0,
        "svg.fonttype": "path",
        "svg.hashsalt": "larmorx",
        "svg.image_inline": True,
        "image.interpolation": "none",
        "mathtext.fontset": "custom",
        "mathtext.rm": SANS,
        "mathtext.it": f"{SANS}:italic",
        "mathtext.bf": f"{SANS}:semibold",
        "mathtext.cal": SANS,
        "mathtext.sf": SANS,
        "mathtext.tt": MONO,
    }


_registered: set[Path] = set()


def register_fonts(cache: Path) -> None:
    """Make the site's fonts known to matplotlib (fetched into the cache on first use)."""
    from matplotlib import font_manager

    from larmorx_validation.figures import fonts

    for path in fonts.ensure(cache):
        if path not in _registered:
            font_manager.fontManager.addfont(str(path))
            _registered.add(path)


def possessive(name: str) -> str:
    """``ANTs'``, ``AFNI's``."""
    return f"{name}'" if name.endswith("s") else f"{name}'s"


def style_axes(ax, theme: Theme, *, grid: str | None = "x") -> None:
    """Recessive axes: no top or right spine, hairline grid along ``grid``."""
    for side in ("top", "right"):
        ax.spines[side].set_visible(False)
    if grid:
        ax.grid(True, axis=grid, color=theme.line_soft, lw=0.75)
        ax.set_axisbelow(True)


def diverging(theme: Theme):
    """Blue below zero, the paper at zero, apricot above."""
    from matplotlib.colors import LinearSegmentedColormap

    return LinearSegmentedColormap.from_list(
        f"lx-diverging-{theme.name}", [theme.replica, theme.paper, theme.larmorx]
    )


def sequential(theme: Theme):
    """From the paper to apricot to ink-ish depth: magnitudes."""
    from matplotlib.colors import LinearSegmentedColormap

    end = "#7a3a08" if theme.name == "light" else "#ffd9b8"
    return LinearSegmentedColormap.from_list(
        f"lx-sequential-{theme.name}", [theme.paper, theme.larmorx, end]
    )
