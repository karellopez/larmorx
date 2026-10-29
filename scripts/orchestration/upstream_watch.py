#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""The upstream watch: is a newer release out for any upstream larmorx ports or follows, and
which of the files the crates' ``PROVENANCE.md`` name did it change?

Usage::

    python -P scripts/orchestration/upstream_watch.py [--workspace W] [--out DIR] [--offline]

For every upstream of ``UPSTREAM.md`` it finds the newest release (``git ls-remote --tags``,
PyPI's JSON API, AFNI's ``AFNI.version``, FSL's public release manifest, the one FSL's installer
reads; without a network, the tags of the workspace's reference clones, read from their
``.git`` files), compares it with the version ``UPSTREAM.md`` names, and for each upstream with a
newer release lists which of the files named in the crates' ``PROVENANCE.md`` (FSL: in
``crates-nc/fsl-source/MANIFEST.tsv``) changed, from the release trees in ``<workspace>/tags/``
(FSL: ``~/fsl-<version>/src``) when they are there; otherwise it says what must be fetched.

It writes ``upstream-watch-<date>.md`` and ``.json`` into ``--out`` (default
``<workspace>/oracles/logs/upstream-watch``) and prints the Markdown. It never changes a
repository and needs no agent: ``difftest-nightly.sh`` runs it once a week (decision log,
2026-10-27, "How versions are handled from now on").
"""

from __future__ import annotations

import argparse
import datetime as dt
import difflib
import hashlib
import json
import os
import re
import subprocess
import sys
import urllib.request
from collections.abc import Callable, Iterable
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
VERSION = re.compile(r"\d+(?:\.\d+)+")
#: File names in PROVENANCE texts: a path or a basename with a source extension.
FILE_TOKEN = re.compile(
    r"`([A-Za-z0-9_.+/-]+?\.(?:c|h|cc|cxx|hxx|txx|cpp|hpp|py|pyx|pxd|niml|S|src|f|inc))"
    r"(?:/\.(?:h|hxx|cxx|txx))*`"
)
#: Directories whose files a bare basename never means.
VENDORED = {"vendored-meson", "tests", "test", "subprojects", "benchmarks"}
#: A changed line that only restates a licence or a copyright (AFNI 26's relicensing).
LICENCE_LINE = re.compile(
    r"Medical College|Wisconsin|General Public License|Gnu General|Creative Commons|CC BY|"
    r"README\.Copyright|[Cc]opyright|[Ll]icen[cs]e|public domain"
)
SPLIT_EXT = re.compile(
    r"`([A-Za-z0-9_./-]+)\.(h|cxx|hxx|txx|c|cc)((?:/\.(?:h|hxx|cxx|txx|c|cc))+)`"
)


@dataclass(frozen=True)
class Upstream:
    """One upstream: where its newest release is found, how ``UPSTREAM.md`` names its current
    one, and where its release trees are kept."""

    name: str
    kind: str  # "git" (release tags), "head" (no releases: a branch's commit), "pypi", "fsl"
    url: str = ""
    tag: str = r"^v(\d+\.\d+\.\d+)$"  # git: a release tag; group 1 is its version
    tags_dirs: tuple[str, ...] = ()  # <workspace>/tags/<dir>; {v} the version, {t} the tag
    clone: str = ""  # the reference clone in <workspace>, for an offline look at its tags
    words: tuple[str, ...] = ()  # PROVENANCE files that mention one of these name its files
    pypi: str = ""
    note: str = ""


UPSTREAMS: tuple[Upstream, ...] = (
    Upstream(
        "ANTs",
        "git",
        "https://github.com/ANTsX/ANTs.git",
        tags_dirs=("ANTs-{t}",),
        clone="ANTs",
        words=("ANTs",),
    ),
    Upstream(
        "ITK",
        "git",
        "https://github.com/InsightSoftwareConsortium/ITK.git",
        tags_dirs=("ITK-{t}",),
        clone="ITK",
        words=("ITK",),
        note="follows ANTs: larmorx ports the ITK that ANTs' newest release builds with",
    ),
    Upstream(
        "ITK remote module AdaptiveDenoising",
        "head",
        "https://github.com/ntustison/ITKAdaptiveDenoising.git",
        note="follows ITK: the commit ITK's release fetches",
    ),
    Upstream(
        "AFNI",
        "git",
        "https://github.com/afni/afni.git",
        tag=r"^AFNI_(\d+\.\d+\.\d+)$",
        tags_dirs=("AFNI-{v}",),
        clone="afni",
        words=("AFNI",),
        note="AFNI releases several times a month: updated in batches unless a release changes "
        "results",
    ),
    Upstream(
        "FreeSurfer",
        "git",
        "https://github.com/freesurfer/freesurfer.git",
        tags_dirs=("freesurfer-{t}", "FreeSurfer-{t}"),
        clone="freesurfer",
        words=("FreeSurfer",),
    ),
    Upstream(
        "CORE-MATH",
        "head",
        "https://gitlab.inria.fr/core-math/core-math.git",
        note="no releases: its master branch",
    ),
    Upstream(
        "GNU C Library",
        "git",
        "https://sourceware.org/git/glibc.git",
        tag=r"^glibc-(\d+\.\d+)$",
        tags_dirs=("glibc-{v}",),
        words=("glibc",),
        note="larmorx follows the glibc the oracles run with (Ubuntu 22.04's 2.35), not the newest",
    ),
    Upstream(
        "OpenBLAS",
        "git",
        "https://github.com/OpenMathLib/OpenBLAS.git",
        tags_dirs=("OpenBLAS-{v}",),
        words=("OpenBLAS",),
    ),
    Upstream(
        "SciPy",
        "pypi",
        pypi="scipy",
        tags_dirs=("scipy-v{v}", "scipy-{v}"),
        words=("SciPy", "scipy"),
    ),
    Upstream(
        "numpy",
        "pypi",
        pypi="numpy",
        tags_dirs=("numpy-{v}",),
        words=("numpy",),
    ),
    Upstream("nibabel", "pypi", pypi="nibabel", tags_dirs=("nibabel-{v}",), words=("nibabel",)),
    Upstream(
        "nitransforms",
        "pypi",
        pypi="nitransforms",
        tags_dirs=("nitransforms-{v}",),
        words=("nitransforms",),
    ),
    Upstream(
        "fMRIPrep",
        "pypi",
        pypi="fmriprep",
        note="a reference for the final comparisons with fMRIPrep, not a version larmorx follows",
    ),
    Upstream(
        "FSL",
        "fsl",
        "https://fsl.fmrib.ox.ac.uk/fsldownloads/fslconda/releases/",
        note="FSL's official build, installed beside the old one (~/fsl-<version>)",
    ),
)


# ------------------------------------------------------------------------------------------------
# Versions


def vkey(version: str) -> tuple[int, ...]:
    return tuple(int(x) for x in re.findall(r"\d+", version))


def current_versions(upstream_md: str) -> dict[str, str]:
    """The version (or commit) ``UPSTREAM.md`` names for each upstream: the first backticked
    tag of its row in "Ported sources", the version of its row in "Behaviour references", and
    FSL's from its section's text."""
    out: dict[str, str] = {}
    for line in upstream_md.splitlines():
        cells = [c.strip() for c in line.split("|")]
        if len(cells) < 5 or cells[1] in ("Project", "---", ""):
            continue
        name, third = cells[1], cells[3]
        ticks = re.findall(r"`([^`]+)`", third)
        if m := VERSION.match(third):  # "2.3.5 (development venv; ...)"
            out[name] = m.group(0)
        elif ticks:
            tag = ticks[0]
            if tag == "master" and len(ticks) > 1:
                tag = ticks[1]  # a branch: its commit
            out[name] = tag
        elif third.startswith("commit") and (m := re.search(r"`([0-9a-f]{7,})`", line)):
            out[name] = m.group(1)
    if m := re.search(r"FSL (\d+(?:\.\d+)+)\s+conda\s+installation", upstream_md):
        out["FSL"] = m.group(1)
    return out


def version_of(up: Upstream, tag: str) -> str:
    """A release's version from its tag (``v2.6.5``, ``AFNI_25.2.09``, ``glibc-2.35``)."""
    if up.kind == "git" and (m := re.match(up.tag, tag)):
        return m.group(1)
    for prefix in ("v", "glibc-"):
        if tag.startswith(prefix) and VERSION.fullmatch(tag[len(prefix) :]):
            return tag[len(prefix) :]
    return tag


def http_get(url: str, timeout: float = 30) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": "larmorx-upstream-watch"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:  # fixed https URLs only
        return resp.read().decode("utf-8", "replace")


def ls_remote(url: str, *args: str) -> list[tuple[str, str]]:
    """``git ls-remote [options] url [patterns]`` (options start with ``-``)."""
    options = [a for a in args if a.startswith("-")]
    patterns = [a for a in args if not a.startswith("-")]
    out = subprocess.run(
        ["git", "ls-remote", *options, url, *patterns],
        capture_output=True,
        text=True,
        timeout=120,
        check=True,
        env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
    ).stdout
    return [tuple(line.split("\t", 1)) for line in out.splitlines() if "\t" in line]  # type: ignore[misc]


def clone_tags(clone: Path) -> list[str]:
    """The tags of a reference clone, read from its ``.git`` files (no git command runs)."""
    git = clone / ".git"
    tags: set[str] = set()
    packed = git / "packed-refs"
    if packed.is_file():
        for line in packed.read_text(encoding="utf-8", errors="replace").splitlines():
            if " refs/tags/" in line:
                tags.add(line.split(" refs/tags/", 1)[1].removesuffix("^{}"))
    loose = git / "refs" / "tags"
    if loose.is_dir():
        tags.update(str(p.relative_to(loose)) for p in loose.rglob("*") if p.is_file())
    return sorted(tags)


@dataclass
class Lookup:
    """How the newest releases are found; replaced in the tests."""

    offline: bool = False
    workspace: Path = REPO.parent
    http: Callable[[str], str] = http_get
    remote: Callable[..., list[tuple[str, str]]] = ls_remote

    def newest(self, up: Upstream) -> tuple[list[str], str, str]:
        """(every release, newest first; the newest; where it was found)."""
        if up.kind == "pypi":
            if self.offline:
                raise LookupError("offline")
            data = json.loads(self.http(f"https://pypi.org/pypi/{up.pypi}/json"))
            releases = [
                v
                for v, files in data.get("releases", {}).items()
                if files and re.fullmatch(r"\d+(?:\.\d+)+", v)
            ]
            releases.sort(key=vkey, reverse=True)
            return releases, data["info"]["version"], "PyPI"
        if up.kind == "fsl":
            if self.offline:
                raise LookupError("offline")
            manifest = json.loads(self.http(up.url + "manifest.json"))
            versions = [v for v in manifest["versions"] if v != "latest"]
            versions.sort(key=vkey, reverse=True)
            return versions, manifest["versions"]["latest"], "FSL's release manifest"
        if up.kind == "head":
            if self.offline:
                raise LookupError("offline")
            refs = self.remote(up.url, "HEAD")
            return [], refs[0][0] if refs else "", "git ls-remote (HEAD)"
        # git release tags
        source = "git ls-remote --tags"
        try:
            if self.offline:
                raise LookupError("offline")
            names = [r.removeprefix("refs/tags/") for _, r in self.remote(up.url, "--tags")]
        except (LookupError, OSError, subprocess.SubprocessError) as err:
            if not up.clone or not (self.workspace / up.clone / ".git").exists():
                raise LookupError(str(err)) from err
            names, source = clone_tags(self.workspace / up.clone), f"the reference clone {up.clone}"
        tags = sorted(
            {n.removesuffix("^{}") for n in names if re.match(up.tag, n.removesuffix("^{}"))},
            key=lambda t: vkey(version_of(up, t)),
            reverse=True,
        )
        newest = tags[0] if tags else ""
        if up.name == "AFNI" and not self.offline:
            try:  # AFNI's own statement of its newest release
                stated = self.http("https://afni.nimh.nih.gov/pub/dist/AFNI.version").split()[0]
                source += f"; AFNI.version says {stated}"
            except (OSError, IndexError):
                pass
        return tags, newest, source


# ------------------------------------------------------------------------------------------------
# The files the crates name


def provenance_tokens(repo: Path, words: Iterable[str]) -> dict[str, set[str]]:
    """``{file token: crates naming it}`` from every ``PROVENANCE.md`` that mentions one of
    ``words`` (a path's tail or a basename)."""
    tokens: dict[str, set[str]] = {}
    for prov in sorted(repo.glob("crates*/*/PROVENANCE.md")):
        text = prov.read_text(encoding="utf-8")
        if not any(re.search(rf"\b{re.escape(w)}\b", text) for w in words):
            continue
        crate = prov.parent.name
        found = set(FILE_TOKEN.findall(text))
        for stem, ext, more in SPLIT_EXT.findall(text):  # `itkFoo.h/.hxx`
            found.add(f"{stem}.{ext}")
            found.update(f"{stem}{e}" for e in re.findall(r"\.\w+", more))
        for tok in found:
            tokens.setdefault(tok.lstrip("./"), set()).add(crate)
    return tokens


def index_tree(root: Path) -> dict[str, list[str]]:
    """``{basename: [relative paths]}`` of a release tree (``.git`` left out)."""
    index: dict[str, list[str]] = {}
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d != ".git"]
        rel = os.path.relpath(dirpath, root)
        for f in filenames:
            index.setdefault(f, []).append(f if rel == "." else f"{rel}/{f}".replace(os.sep, "/"))
    return index


def changed_lines(a: bytes, b: bytes) -> tuple[int, bool]:
    """(changed lines, whether every one is licence or copyright text)."""
    la = a.decode("latin-1").splitlines()
    lb = b.decode("latin-1").splitlines()
    lines = [
        line[1:]
        for line in difflib.unified_diff(la, lb, lineterm="", n=0)
        if line[:1] in "+-" and not line.startswith(("+++", "---"))
    ]
    licence = all(LICENCE_LINE.search(x) or not x.strip() for x in lines)
    return len(lines), bool(lines) and licence


def compare_trees(old: Path, new: Path, tokens: dict[str, set[str]]) -> dict:
    """The named files of ``old`` and what ``new`` did to them."""
    index = index_tree(old)
    named: dict[str, set[str]] = {}
    for tok, crates in tokens.items():
        base = tok.rsplit("/", 1)[-1]
        for rel in index.get(base, []):
            if "/" in tok and not rel.endswith(tok):
                continue
            # A bare basename (``base.py``) is not a vendored copy or a test of another project.
            if "/" not in tok and set(rel.split("/")[:-1]) & VENDORED:
                continue
            named.setdefault(rel, set()).update(crates)
    changed, removed, same = [], [], 0
    for rel in sorted(named):
        a, b = old / rel, new / rel
        if not b.is_file():
            removed.append({"file": rel, "crates": sorted(named[rel])})
            continue
        da, db = a.read_bytes(), b.read_bytes()
        if da == db:
            same += 1
        else:
            n, licence = changed_lines(da, db)
            changed.append(
                {"file": rel, "lines": n, "licence_only": licence, "crates": sorted(named[rel])}
            )
    return {"named": len(named), "unchanged": same, "changed": changed, "removed": removed}


def fsl_manifest_rows(repo: Path) -> list[tuple[str, str, str, str]]:
    path = repo / "crates-nc" / "fsl-source" / "MANIFEST.tsv"
    if not path.is_file():
        return []
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) == 4:
            rows.append((parts[0], parts[1], parts[2], parts[3]))
    return rows


def compare_fsl(repo: Path, new_src: Path) -> dict:
    """FSL's replicated files (``MANIFEST.tsv``: package, version, file, SHA-256) against a new
    installation's ``src/<package>/<file>``."""
    changed, removed, same = [], [], 0
    rows = fsl_manifest_rows(repo)
    for pkg, _ver, rel, sha in rows:
        p = new_src / pkg / rel
        if not p.is_file():
            removed.append({"file": f"{pkg}/{rel}", "crates": ["larmorx-nc"]})
        elif hashlib.sha256(p.read_bytes()).hexdigest() == sha:
            same += 1
        else:
            changed.append(
                {
                    "file": f"{pkg}/{rel}",
                    "lines": None,
                    "licence_only": False,
                    "crates": ["larmorx-nc"],
                }
            )
    return {"named": len(rows), "unchanged": same, "changed": changed, "removed": removed}


def fsl_packages(lookup: Lookup, version: str) -> dict[str, str]:
    """``{package: version}`` of an FSL release (its linux-64 environment file)."""
    text = lookup.http(f"{UPSTREAMS[-1].url}fsl-{version}_linux-64.yml")
    return dict(re.findall(r"^\s*-\s*(fsl[\w.-]*)\s+([\w.]+)\s*$", text, flags=re.M))


# ------------------------------------------------------------------------------------------------
# The watch


@dataclass
class Result:
    name: str
    current: str
    newest: str = ""
    status: str = "unknown"  # "newest", "newer", "unknown"
    source: str = ""
    newer: list[str] = field(default_factory=list)
    files: dict | None = None
    fetch: str = ""
    note: str = ""
    error: str = ""
    packages: dict | None = None
    current_version: str = ""

    def changed_list(self) -> list[dict]:
        return [] if self.files is None else self.files["changed"]


def tree_for(workspace: Path, up: Upstream, version: str, tag: str) -> Path | None:
    for pattern in up.tags_dirs:
        p = workspace / "tags" / pattern.format(v=version, t=tag)
        if p.is_dir():
            return p
    return None


def watch_one(up: Upstream, current: str, lookup: Lookup, repo: Path, fsl_root: Path) -> Result:
    r = Result(up.name, current, note=up.note)
    try:
        releases, newest, r.source = lookup.newest(up)
    except (LookupError, OSError, ValueError, KeyError, subprocess.SubprocessError) as err:
        r.error = f"{type(err).__name__}: {err}"
        return r
    if up.kind == "head":
        r.newest = newest[:12]
        r.status = (
            "newest" if newest.startswith(current) or current.startswith(newest[:12]) else "newer"
        )
        if r.status == "newer":
            r.fetch = f"the branch moved from {current} to {newest[:12]}: fetch it to compare"
        return r
    cur_v = version_of(up, current)
    r.current_version = cur_v
    r.newest = version_of(up, newest)
    if not VERSION.fullmatch(cur_v):
        r.status = "newer" if r.newest else "unknown"
        r.fetch = f"{current} is not a release; the newest release is {r.newest}"
        return r
    r.newer = [version_of(up, t) for t in releases if vkey(version_of(up, t)) > vkey(cur_v)]
    if vkey(r.newest) <= vkey(cur_v):
        r.status = "newest"
        return r
    r.status = "newer"
    if up.kind == "fsl":
        try:
            old_p, new_p = fsl_packages(lookup, cur_v), fsl_packages(lookup, r.newest)
            names = sorted({pkg for pkg, *_ in fsl_manifest_rows(repo)} | {"fslpy"})
            r.packages = {
                n: [old_p.get(n, "?"), new_p.get(n, "?")]
                for n in names
                if old_p.get(n) != new_p.get(n)
            }
        except (OSError, ValueError) as err:
            r.error = f"package lists: {err}"
        new_src = fsl_root / f"fsl-{r.newest}" / "src"
        if new_src.is_dir():
            r.files = compare_fsl(repo, new_src)
        else:
            r.fetch = f"install FSL {r.newest} beside the old one (~/fsl-{r.newest}) to compare"
        return r
    if not up.words:
        return r
    newest_tag = newest
    old_tree = tree_for(lookup.workspace, up, cur_v, current)
    new_tree = tree_for(lookup.workspace, up, r.newest, newest_tag)
    missing = [
        f"<workspace>/tags/{up.tags_dirs[0].format(v=v, t=t)}"
        for v, t, tree in ((cur_v, current, old_tree), (r.newest, newest_tag, new_tree))
        if tree is None
    ]
    if missing:
        r.fetch = "fetch " + " and ".join(missing) + (f" (tag {newest_tag})" if up.url else "")
        return r
    assert old_tree is not None and new_tree is not None
    r.files = compare_trees(old_tree, new_tree, provenance_tokens(repo, up.words))
    return r


def run_watch(
    repo: Path, lookup: Lookup, fsl_root: Path, only: Iterable[str] | None = None
) -> list[Result]:
    currents = current_versions((repo / "UPSTREAM.md").read_text(encoding="utf-8"))
    wanted = set(only or [])
    results = []
    for up in UPSTREAMS:
        if wanted and up.name not in wanted:
            continue
        current = currents.get(up.name, "")
        if not current:
            results.append(Result(up.name, "", error="not named in UPSTREAM.md", note=up.note))
            continue
        results.append(watch_one(up, current, lookup, repo, fsl_root))
    return results


def render(results: list[Result], when: str) -> str:
    lines = [
        f"# Upstream watch, {when}",
        "",
        "Made by `scripts/orchestration/upstream_watch.py`: the newest release of every upstream "
        "of `UPSTREAM.md`, and for a newer one the files the crates' `PROVENANCE.md` name that it "
        "changed (the release trees in `<workspace>/tags/`, FSL's in `~/fsl-<version>/src`).",
        "",
        "| Upstream | larmorx | Newest | Status | Named files changed | Found by |",
        "|---|---|---|---|---|---|",
    ]
    for r in results:
        if r.files is not None:
            changed = f"{len(r.changed_list())} of {r.files['named']}"
            if lic := sum(1 for c in r.changed_list() if c.get("licence_only")):
                changed += f" ({lic} licence text only)"
            if r.files["removed"]:
                changed += f" ({len(r.files['removed'])} removed)"
        elif r.fetch:
            changed = "not compared (below)"
        else:
            changed = "–"
        status = {"newest": "newest", "newer": "**newer release**", "unknown": "unknown"}[r.status]
        if r.error and r.status == "unknown":
            status = f"unknown ({r.error[:60]})"
        lines.append(
            f"| {r.name} | {r.current} | {r.newest or '?'} | {status} | {changed} | {r.source} |"
        )
    for r in results:
        if r.status != "newer":
            continue
        lines += ["", f"## {r.name}: {r.current_version or r.current} → {r.newest}", ""]
        if r.note:
            lines.append(f"- {r.note}.")
        if r.newer:
            shown = ", ".join(r.newer[:12]) + (" ..." if len(r.newer) > 12 else "")
            lines.append(
                f"- {len(r.newer)} newer release(s): {shown}; compared: "
                f"{r.current_version or r.current} against {r.newest} (cumulative)."
            )
        if r.packages:
            pk = ", ".join(f"{n} {a} → {b}" for n, (a, b) in sorted(r.packages.items()))
            lines.append(f"- packages of the replicated code that changed version: {pk}.")
        if r.fetch:
            lines.append(f"- not compared: {r.fetch}.")
        if r.files is not None:
            f = r.files
            lines.append(
                f"- {f['named']} named files: {f['unchanged']} unchanged, "
                f"{len(f['changed'])} changed, {len(f['removed'])} removed."
            )
            for c in sorted(f["changed"], key=lambda c: (bool(c.get("licence_only")), c["file"])):
                n = "" if c["lines"] is None else f", {c['lines']} lines"
                lic = ", licence text only" if c.get("licence_only") else ""
                lines.append(f"  - changed: `{c['file']}`{n}{lic} ({', '.join(c['crates'])})")
            for c in f["removed"]:
                lines.append(f"  - removed: `{c['file']}` ({', '.join(c['crates'])})")
    errors = [r for r in results if r.error]
    if errors:
        lines += ["", "## Not reached", ""]
        lines += [f"- {r.name}: {r.error}" for r in errors]
    return "\n".join(lines) + "\n"


def find_workspace() -> Path:
    if env := os.environ.get("LARMORX_WORKSPACE"):
        return Path(env)
    for parent in REPO.parents:
        if (parent / "tags").is_dir() or (parent / "oracles").is_dir():
            return parent
    return REPO.parent


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--workspace", type=Path, default=None)
    ap.add_argument("--repo", type=Path, default=REPO)
    ap.add_argument("--out", type=Path, default=None)
    ap.add_argument("--fsl-root", type=Path, default=Path.home())
    ap.add_argument("--offline", action="store_true", help="the reference clones' tags only")
    ap.add_argument("--only", nargs="*", help="these upstreams only (names as in UPSTREAM.md)")
    args = ap.parse_args(argv)
    workspace = args.workspace or find_workspace()
    lookup = Lookup(offline=args.offline, workspace=workspace)
    results = run_watch(args.repo, lookup, args.fsl_root, args.only)
    when = dt.date.today().isoformat()
    text = render(results, when)
    out = args.out or workspace / "oracles" / "logs" / "upstream-watch"
    out.mkdir(parents=True, exist_ok=True)
    (out / f"upstream-watch-{when}.md").write_text(text, encoding="utf-8")
    payload = {"date": when, "upstreams": [r.__dict__ for r in results]}
    (out / f"upstream-watch-{when}.json").write_text(
        json.dumps(payload, indent=1) + "\n", encoding="utf-8"
    )
    sys.stdout.write(text)
    print(f"\nwritten: {out / f'upstream-watch-{when}.md'} (and .json)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
