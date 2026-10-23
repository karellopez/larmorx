# Brief: Build larmorx_documentation landing

As given to agent `a681512e6d7e8168e` on 2026-10-20, verbatim (paths are this machine's).

You are building the first version of LARMORX's documentation site, in the new local git repository `/home/karelo/VScode_projects/super_fmriprep/larmorx_documentation` (branch `main`, empty, git identity already configured). Work there directly. The project it documents is `/home/karelo/VScode_projects/super_fmriprep/larmorx`: read its `README.md` (identity, tagline, philosophy, supported tools, roadmap) and `CLAUDE.md` (naming: the acronym LARMORX in prose, lowercase `larmorx` in code and commands).

## The style to follow
The user wants the style of their lab's site `bids_manager_documentation`, cloned read-only at
`/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/ref-bm/bids_manager_documentation`. Study it (`static/css/site.css`, `templates/`, `static/js/home.js`, `tools/build.py`, `tools/hero/make_hero.py`, `content/index.md`, `README.md`, `.github/workflows/pages.yml`) and reproduce its **design language**, not its code: that repository has no licence file, so write your own implementation. Its fonts are under the SIL Open Font License and may be copied, with their OFL texts.
The design language:
- printed matter: warm paper in both themes (warm light grey by day, warm grey-black by night), one fixed grain layer over everything;
- light and dark themes, following the system by default, with a switch;
- one pastel palette, the same in both themes, for fills, marks and hero points, deeper variants where pastels are too pale on light paper; text always ink;
- Instrument Serif for titles and numerals, Inter for reading, JetBrains Mono for kickers, labels and names;
- hairline separators with a short pastel mark; generous whitespace; no ornament for its own sake;
- accessible: contrast, keyboard navigation, `prefers-reduced-motion`, an effects switch, a still image without JavaScript or WebGL2.
For LARMORX, give each software ecosystem its own pastel (ANTs, AFNI, FSL, FreeSurfer, Workbench, plus one for LARMORX itself as the accent).

## What to build (small, solid, extensible)
1. **The builder:** `tools/build.py` (Python: Jinja2, markdown-it-py, PyYAML; a `requirements.txt`), reading `site.yml` (title, description, base URL `https://karellopez.github.io/larmorx_documentation/`, repository links, navigation), `content/*.md` with front matter, `templates/` (`base.html`, `home.html`, `doc.html`, `404`), `static/` (css, js, fonts, hero, brand); output `_site/` (git-ignored); `--serve` for preview; link and anchor checks that stop the build. `.github/workflows/pages.yml` builds and deploys to GitHub Pages on push to `main`. A `README.md` for the repository. Use a virtual environment inside the repository (`.venv/`, git-ignored) for the build; numpy and nibabel are in `/home/karelo/VScode_projects/super_fmriprep/.venv` for the hero precomputation.
2. **The landing page** (`content/index.md` + `templates/home.html`):
   - the hero: "LARMORX", the acronym expanded (Library of Algorithmic Reimplementations for MRI through Open Rust eXecution), the tagline "Bringing Neuroimaging Tools into Resonance." and "Reimplemented in Rust. Accessible through Python. Built for FAIR research.", buttons to the GitHub repository and to the sections below;
   - below it, short sections: what LARMORX is; core features; the philosophy (the Larmor frequency, ω = γB₀, and FAIR4RS); current status, keeping implemented and planned apart exactly as the README does (no claims beyond it; link to the validation records in the main repository).
   - One small placeholder documentation page (e.g. "Validation") so the doc template exists.
3. **The hero animation, which represents LARMORX** (WebGL2, your own code, in the spirit of the reference's particle hero):
   - Particles are spins. Each belongs to one ecosystem (coloured by its pastel). At the top of the page they precess about a vertical B₀ axis at slightly different frequencies and random phases: incoherent, dephased.
   - As the reader scrolls, their frequencies converge on one Larmor frequency and their phases lock: **resonance**. The cloud becomes coherent and assembles into a 3-D brain, a point cloud sampled from the MNI ICBM152 2009c template (`/home/karelo/VScode_projects/super_fmriprep/oracles/templateflow/tpl-MNI152NLin2009cAsym/`, e.g. the grey-matter probability or the brain surface at res-01), turning slowly, with the shared precession as a subtle shimmer. The ecosystems' colours stay visible in the brain: many toolkits, one framework.
   - A quiet heads-up display in the mono font: the coherence (|mean e^{iφ}|, from about 0 to 1) and ω = γB₀; a thin B₀ arrow.
   - Precompute the points with `tools/hero/make_hero.py` into a compact binary (`static/hero/hero.bin`) and still frames for light and dark (`hero-light.svg`, `hero-dark.svg`) shown without WebGL2, with reduced motion, or with effects off. Credit the MNI template (its copyright notice) in the footer or a credits note.
   - Smooth at 60 fps on a laptop, adaptive on slow machines (resolution first, then particle count), and good on a phone in portrait.
4. **Visual review:** take screenshots with headless Chrome (`/usr/bin/google-chrome`) at desktop (1440×900) and phone (390×844) widths, light and dark, at several scroll positions of the hero, look at them, and iterate until the result is elegant and professional. Keep the final screenshots in `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/docs-site/shots/`.
5. **A standalone preview:** `_preview/index.html` (git-ignored), the landing page as one self-contained file: CSS and JavaScript inline, the hero data as a data URI, fonts from Google Fonts (Inter, Instrument Serif, JetBrains Mono), under 5 MB, so the orchestrator can publish it as a private preview page the user can open on a phone.

## Working rules
- Commit early and often in `larmorx_documentation`; the git identity is configured; never add `Co-Authored-By`, "Generated with" or any AI attribution. No remote exists yet: do not push.
- Do not modify the main `larmorx` repository or the reference clone. Never delete work products.
- Scratch files in `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/docs-site/`, with a short `NOTES.md` (done / next) in case of interruption.
- You are the only agent. The user watches the weekly usage limit: work efficiently, and finish (do not stop silently).

## Report (concise)
What was built, how the hero works and what it shows, the screenshots' paths, how to build and preview, the preview file's path and size, commits.

