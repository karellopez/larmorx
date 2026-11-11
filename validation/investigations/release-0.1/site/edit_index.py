# SPDX-License-Identifier: Apache-2.0
"""The home page's first edit for the first release (relay 5b, 2026-11-11; site commit 5650433),
kept as it ran: applied to `content/index.md` of the site's main at d9f67d1, it replaces the
"not yet on PyPI" lines (the hero's line, the Status margin note, the closing status note) and
the cards that had gone stale (the three ways, the ecosystems, FAIR's Findable and Accessible,
the status section's versions). Each replacement must apply exactly once. The later site commit
af701b6 put the replica card's two licences on two lines; the page on release-pages is the
result.

Usage: python -P edit_index.py <site checkout at d9f67d1>
"""

import sys
from pathlib import Path

R = [
    (
        "hero_meta: Apache-2.0 · Linux, macOS and Windows · not yet on PyPI",
        "hero_meta: Apache-2.0 · Linux, macOS and Windows · pip install larmorx",
    ),
    (
        """<p class="margin-note"><b>Status</b>Early development. The tools listed under <a href="#status">Status</a> are implemented and validated against the original programs; everything else is planned. Not yet on PyPI: <a href="{{ repo('README.md') }}#development">build it from source</a>.</p>""",
        """<p class="margin-note"><b>Status</b>Early development; the first release, 0.1.0rc1, is a pre-release. The tools listed under <a href="#status">Status</a> are implemented and validated against the original programs; everything else is planned. How to install it: <a href="{{ link('/installation/') }}">installation</a>.</p>""",
    ),
    (
        """<p>Where the licence is restrictive (AFNI's GPL code, FSL), an original is written from a behavioural specification, documentation and runs of the original program, never from its source.</p>""",
        """<p>Where the licence is restrictive (FSL, tedana, Workbench), an original is written from a behavioural specification, documentation and runs of the original program, never from its source.</p>""",
    ),
    (
        """<p>An exact replica, where one exists, is a separate package under the upstream's licence, such as <code>larmorx-gpl</code>, that runs as a separate program. Tools with both take <code>implementation="auto" | "replica" | "original"</code>.</p><p class="where">larmorx-gpl · GPL-3.0-or-later</p>""",
        """<p>An exact replica, where one exists, is a separate package under the upstream's licence (<code>larmorx-nc</code> for FSL, <code>larmorx-gpl</code> for tedana's <code>t2smap</code>) that runs as a separate program. AFNI's tools have both inside LARMORX since AFNI 26.3.05 released the Medical College of Wisconsin's code under CC BY 4.0. Tools with both take <code>implementation="auto" | "replica" | "original"</code>.</p><p class="where">larmorx-nc · FSL Licence · larmorx-gpl · GPL-3.0</p>""",
    ),
    (
        """<p>Ported from the source: transforms, N4, denoising, ImageMath, registration (linear and deformable stages).</p>""",
        """<p>Ported from the source: transforms, N4, denoising, ImageMath and its filters, registration (linear and deformable stages), antsAI, Atropos.</p>""",
    ),
    (
        """<p>3dTshift as a clean-room original, with an exact replica in <code>larmorx-gpl</code>.</p>""",
        """<p>3dTshift, 3dAutomask, 3dUnifize and 3dvolreg, each an exact replica and a clean-room original, in <code>larmorx</code>.</p>""",
    ),
    (
        """<p>Head-motion correction with mcflirt-style options, a clean-room original, experimental.</p>""",
        """<p>mcflirt, BET, FLIRT, FAST, topup, <code>fslmaths</code>, <code>fslstats</code> and prelude: clean-room originals in <code>larmorx</code>, exact replicas in <code>larmorx-nc</code>.</p>""",
    ),
    (
        """<p>Its own NIfTI I/O and a port of fMRIPrep's one-shot BOLD resampler; the larmorprepx pipeline is planned.</p>""",
        """<p>Its own NIfTI I/O, ports of fMRIPrep's one-shot BOLD resampler and of sdcflows' field-map estimation, and tedana's <code>t2smap</code> (an exact replica in <code>larmorx-gpl</code>); the larmorprepx pipeline is planned.</p>""",
    ),
    (
        """<p>A public repository with versioned history; PyPI releases with citation metadata are planned.</p>""",
        """<p>A public repository with versioned history, releases on PyPI and GitHub, and citation metadata (<a class="body-link" href="{{ repo('CITATION.cff') }}">CITATION.cff</a>).</p>""",
    ),
    (
        """<p>Open source under Apache-2.0; native builds with no external neuroimaging installations, containers or administrator rights.</p>""",
        """<p>Open source under Apache-2.0; native wheels for six platforms that need no external neuroimaging installations, containers or administrator rights, and <a class="body-link" href="{{ link('/installation/') }}#containers">container images</a> for those who prefer them.</p>""",
    ),
    (
        """Implemented and validated against the original programs (ANTs 2.6.5, AFNI 25.2.09, FSL 6.0.7, nibabel).""",
        """Implemented and validated against the original programs (ANTs 2.6.5, AFNI 26.3.05, FSL 6.0.7.23, sdcflows 2.17.1, tedana 26.0.3, nibabel).""",
    ),
    (
        """<p class="status-note">LARMORX is not on PyPI yet. The planned distribution is <code>pip install larmorx</code> for the library and the <code>larmorx</code> and <code>lx</code> commands, with extras for exact replicas of GPL-licensed tools (<code>larmorx[exact]</code>) and the larmorprepx pipeline (<code>larmorx[prep]</code>). Until then, <a class="body-link" href="{{ repo('README.md') }}#development">build it from source</a>.</p>""",
        """<p class="status-note">Install it with <code>pip install larmorx</code>: the library and the <code>larmorx</code> and <code>lx</code> commands. <code>larmorx[prep]</code> adds what the larmorprepx pipeline will need, and <code>larmorx[all]</code> also the exact replicas of FSL's tools (<code>larmorx-nc</code>, non-commercial use only) and of tedana's <code>t2smap</code> (<code>larmorx-gpl</code>, GPL-3.0). Standalone programs, container images and what your computer needs: <a class="body-link" href="{{ link('/installation/') }}">installation</a>.</p>""",
    ),
]


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        raise SystemExit(__doc__)
    path = Path(argv[0]) / "content/index.md"
    text = path.read_text(encoding="utf-8")
    for old, new in R:
        if text.count(old) != 1:
            raise SystemExit(f"not found once: {old[:80]}")
        text = text.replace(old, new)
    path.write_text(text, encoding="utf-8")
    print(len(R), "replacements")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
