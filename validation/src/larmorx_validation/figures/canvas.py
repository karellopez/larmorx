# SPDX-License-Identifier: Apache-2.0
"""A figure laid out in CSS pixels: text, swatches and bars placed exactly, and real axes where
a figure has a scale. ``y`` grows downwards, as on the page."""

from __future__ import annotations

from functools import cache

from larmorx_validation.figures import style


@cache
def _font(family: str, weight: int, slant: str):
    from matplotlib.font_manager import FontProperties

    return FontProperties(family=family, weight=weight, style=slant)


def text_width(s: str, family: str = style.SANS, size: float = 12, weight: int = 400) -> float:
    """The width of ``s`` in pixels at ``size`` pixels."""
    from matplotlib.textpath import TextPath

    if not s:
        return 0.0
    path = TextPath(
        (0, 0), s.replace(" ", "\u00a0"), size=size, prop=_font(family, weight, "normal")
    )
    ext = path.get_extents()
    return float(ext.x1) if ext.width > 0 else 0.0


def wrap(s: str, width: float, family: str = style.SANS, size: float = 12) -> list[str]:
    words, lines, cur = s.split(), [], ""
    for w in words:
        trial = f"{cur} {w}".strip()
        if cur and text_width(trial, family, size) > width:
            lines.append(cur)
            cur = w
        else:
            cur = trial
    if cur:
        lines.append(cur)
    return lines


class Canvas:
    def __init__(self, theme: style.Theme, width: float, height: float):
        import matplotlib.pyplot as plt

        self.theme, self.W, self.H = theme, width, height
        self.fig = plt.figure(figsize=(style.inches(width), style.inches(height)))
        ax = self.fig.add_axes((0, 0, 1, 1))
        ax.set_xlim(0, width)
        ax.set_ylim(height, 0)
        ax.axis("off")
        self.ax = ax

    def text(
        self,
        x: float,
        y: float,
        s: str,
        *,
        family: str = style.SANS,
        size: float = 12,
        color: str | None = None,
        ha: str = "left",
        va: str = "baseline",
        weight: int = 400,
        **kw,
    ):
        return self.ax.text(
            x,
            y,
            s,
            fontfamily=family,
            fontsize=style.pt(size),
            color=color or self.theme.ink,
            ha=ha,
            va=va,
            fontweight=weight,
            **kw,
        )

    def rect(
        self,
        x: float,
        y: float,
        w: float,
        h: float,
        color: str,
        *,
        hatch: str | None = None,
        edge: str | None = None,
        alpha: float = 1.0,
        lw: float = 0.0,
    ):
        from matplotlib.patches import Rectangle

        r = Rectangle(
            (x, y),
            w,
            h,
            facecolor=color,
            edgecolor="none" if hatch else (edge or "none"),
            hatch=hatch,
            hatchcolor=edge or self.theme.ink3,
            linewidth=lw,
            alpha=alpha,
        )
        self.ax.add_patch(r)
        return r

    def hline(self, x0: float, x1: float, y: float, *, color=None, lw: float = 0.75):
        self.ax.plot([x0, x1], [y, y], color=color or self.theme.line, lw=lw, solid_capstyle="butt")

    def axes(self, x: float, y: float, w: float, h: float):
        """Real axes occupying the box (x, y, w, h) in pixels from the top left."""
        return self.fig.add_axes((x / self.W, 1 - (y + h) / self.H, w / self.W, h / self.H))

    def swatch(self, x: float, y: float, state: dict, size: float = 11):
        self.rect(
            x, y, size, size, state["color"], hatch=state.get("hatch"), edge=state.get("edge")
        )
