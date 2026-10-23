# Final report: Build larmorx_documentation landing

Agent `a681512e6d7e8168e`, 2026-10-20. The agent's report to the orchestrator, verbatim.

The first version of the LARMORX documentation site is built and committed in `/home/karelo/VScode_projects/super_fmriprep/larmorx_documentation` (branch `main`, 3 commits, not pushed). It builds with 0 problems, I reviewed it in headless Chrome over several rounds, and the standalone preview is ready to publish.

**What was built**
- **Builder**, `tools/build.py`: it reads `site.yml`, `data/*.yml` and `content/*.md` (front matter, then Jinja, then Markdown), renders `templates/` and copies `static/` into `_site/`.
  - Every build checks internal links and anchors, one h1 per page, heading levels that don't skip, and template leftovers. It also checks that the main-repository files that pages link to exist, when `../larmorx` is checked out next to it (30 links). Any problem stops the build.
  - I tested the check with a deliberately broken page: a bad link, a bad anchor, a missing repo file and a skipped heading level were all reported, with exit status 1.
  - `--serve` serves under `/larmorx_documentation/` and rebuilds on change. `--preview` writes the standalone file.
- **Other repository files:** `requirements.txt`, a `.venv/` inside the repo (git-ignored), `.github/workflows/pages.yml` (build and deploy to Pages on push to `main`), and a `README.md`.
- **Design**, in my own code, not the reference's (`static/css/site.css`):
  - warm paper in both themes with one fixed grain layer; light and dark themes that follow the system, with a System/Light/Dark menu;
  - an Effects switch, off by default under `prefers-reduced-motion`; skip link, focus outlines, keyboard menu;
  - one pastel per ecosystem (ANTs, AFNI, FSL, FreeSurfer, Workbench), apricot as the LARMORX accent, and deeper variants (3:1 against the paper) for marks on light paper;
  - Instrument Serif, Inter and JetBrains Mono, copied with their OFL texts.
- **Home page**: the hero (LARMORX, the acronym with its initials underlined, the tagline, "Reimplemented in Rust…", buttons to GitHub and to the sections below), then four sections.
  - 01 What it is: ported, clean-room original, exact replica, and the ecosystem cards.
  - 02 Core features, with the README's CLI and Python examples.
  - 03 Philosophy: ω = γB₀ and FAIR4RS.
  - 04 Status: 12 implemented rows and 6 planned items, taken from the README and kept apart as it does, each row linking to its validation record. They live in `data/status.yml`.
- **Doc page**: `/validation/`, with sidebar and table of contents. There is also a 404 page.

**How the hero works** (`static/js/hero.js`, WebGL2; data from `tools/hero/make_hero.py`)
- **The brain:** 36,000 points sampled from MNI ICBM152 2009c at res-01. TemplateFlow's grey-matter probability file is an empty placeholder here, so I took the grey matter from the T1w intensities inside the brain mask, weighted towards the outer cortex, plus the mask surface. They are stored in `static/hero/hero.bin` (282 KB), with still drawings `hero-light.svg` and `hero-dark.svg`.
- **Colours in the brain:** each ecosystem holds patches of the brain, cells grown from random seeds and coloured so neighbours differ. No atlas is used: the Harvard-Oxford labels in TemplateFlow carry FSL licence questions, so I avoided them.
- **Top of the page:** five ensembles (rings), one per ecosystem, precess about a vertical B₀ axis, each at its own frequency (0.66 to 1.34 ω₀) with random phases. Coherence is about 0.02.
- **Scrolling:** the frequencies converge, the phases lock, and the spins swirl into the brain, which turns slowly about B₀ with a faint shimmer and keeps the five colours. At the end a "Many toolkits, *one* framework." title and a legend appear.
- **Readout and arrow:** a mono line shows the measured coherence |⟨e^{iφ}⟩| (rising to 1.00), the frequency spread and ω = γB₀. A thin B₀ arrow runs through the middle and draws back above the brain as it forms. I built the phase formula so nothing jumps however fast the reader scrolls.
- **Fallbacks:** without WebGL2, with effects off or without JavaScript, the page shows the still drawing and a legend; I checked all three.
- **Performance:** I could only test on software rendering (SwiftShader), so 60 fps on a real laptop GPU and a real phone are unmeasured. On SwiftShader the step-down worked (resolution first, then half the points, at level 2). The vertex shader does only 4 trig calls per point.
- **Credits:** the MNI copyright notice is in the footer and in `static/hero/LICENSE-MNI.txt`.

**Screenshots**: 48 final PNGs in `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/docs-site/shots/`. They cover the home hero at 0, 0.3, 0.6 and 1, the page body, validation and still mode, at 1440×900 and 390×844, light and dark. Earlier review rounds are in `shots/rounds/`. `tools/review/shoot.mjs` reproduces them (Node 22 and Chrome).

**Build and preview**
```
cd larmorx_documentation
.venv/bin/python tools/build.py --serve     # http://localhost:8765/larmorx_documentation/
.venv/bin/python tools/build.py --preview   # also writes _preview/index.html
```

**Preview file**: `/home/karelo/VScode_projects/super_fmriprep/larmorx_documentation/_preview/index.html`, 0.92 MB (git-ignored). CSS and JS are inline, the hero data is a data URI, fonts come from Google Fonts, and links to other pages point to the future `karellopez.github.io/larmorx_documentation/` address. I checked it from `file://`: the hero runs and the fonts load.

**Commits**: `e2a59ae` (first version), `d2c8901` (hero performance and layout at every width, review tool options), `1257c05` (small hero clean-up). They carry the configured author and no AI attribution.

Points for review:
- Greek letters (ω, γ) fall back to a system serif, because the copied font subsets lack Greek.
- On phones without JavaScript there is no navigation menu; the page is one long page with anchors.
- If both main repositories are public, Pages must be enabled once in the repository's settings (Source: GitHub Actions).
