# SPDX-License-Identifier: Apache-2.0
"""The key of a run of an original program: a SHA-256 over everything its result depends on,
and nothing it does not.

The parts (``Prepared.parts``, kept readable in every entry):

- **the oracle**: the program's SHA-256 and the SHA-256 of every shared library it loads
  (``ldd``: glibc's ``libm`` decides the unmodified programs' maths); for a script, the build's
  (every file beside it); for a container, the image's SHA-256, the program's name and
  Apptainer's version;
- **the arguments**, with every path replaced: a file that exists is an **input**, named by its
  SHA-256 (with the files read beside it: ``.img`` for ``.hdr``, ``.BRIK`` for ``.HEAD``) and its
  suffix (programs choose a reader by it), never by its directory or name; a directory the run
  writes in (a declared output, the working directory, or a temporary directory the arguments
  name or will create a file in) is a **place**, named by its role (``{cwd}``, ``{out0}``,
  ``{d0}``, ...), with the path below it kept;
- **the environment that matters**: thread counts and seeds (``ITK_*``, ``NSLOTS``, ``OMP_*``,
  ``ANTS_*``), AFNI's and FSL's variables, ``LD_PRELOAD`` (by its files' SHA-256),
  ``LD_LIBRARY_PATH``, ``GLIBC_TUNABLES``, the locale, ``TZ``; ``HOME`` by its contents when it
  is a small directory (AFNI's runs use an empty one);
- **the machine**: the architecture and the CPU's model and flags (glibc picks its maths
  kernels by them), and the number of CPUs when the run's thread count is not set.

What is left out on purpose: the larmorx commit (the original does not depend on it), directory
and file names, ``PATH``, the time.
"""

from __future__ import annotations

import functools
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import tempfile
from collections.abc import Sequence
from dataclasses import dataclass, field, replace
from pathlib import Path

from larmorx_validation.oracle_cache.hashing import (
    directory_digest,
    sha256_content,
    sha256_file,
)

KEY_FORMAT = 1

#: Variables in the key of every program (a name ending in ``_`` or ``*`` is a prefix): the
#: dynamic loader's and glibc's (they choose the code that runs), the locale and the time zone.
COMMON_ENV = (
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "GLIBC_TUNABLES",
    "MALLOC_",
    "LANG",
    "LANGUAGE",
    "LC_",
    "TZ",
)
#: The variables each family of programs reads, besides :data:`COMMON_ENV`. ITK also reads the
#: variables ``ITK_NUMBER_OF_THREADS_ENV_LIST`` names.
ENV_FAMILIES = {
    "itk": ("ITK_", "ANTS_", "NSLOTS"),
    "afni": ("AFNI_", "NIML_", "OMP_", "GOMP_", "KMP_", "HOME"),
    "fsl": ("FSL*", "OMP_", "GOMP_"),
}


#: Files programs write into ``HOME`` and never read (left out of its digest): AFNI's crash log
#: and log file.
WRITTEN_IN_HOME = (".afni.crashlog", ".afni.log")


def env_names(family: str | None) -> tuple[str, ...]:
    """The variables in the key of a program of ``family`` (``None``: of any family)."""
    if family is None:
        return COMMON_ENV + tuple(n for names in ENV_FAMILIES.values() for n in names)
    return COMMON_ENV + ENV_FAMILIES[family]


def env_matters(name: str, family: str | None = None, env: dict[str, str] | None = None) -> bool:
    """Whether the variable ``name`` is part of the key of a program of ``family``."""
    for n in env_names(family):
        if name == n or (n.endswith(("_", "*")) and name.startswith(n.rstrip("*"))):
            return True
    listed = (env or {}).get("ITK_NUMBER_OF_THREADS_ENV_LIST", "")
    return family in (None, "itk") and name in listed.split(":")


#: Characters that separate paths inside an argument (``MI[a,b,1,32]``, ``-x [m1,m2]``,
#: ``x.nii.gz[0..3]``, ``@file.1D``, ``--name=value``).
_SPLIT = re.compile(r"""([\s,\[\];=:@'"(){}<>|]+)""")

#: Known file suffixes, longest first (the reader a program picks).
SUFFIXES = (
    ".nii.gz",
    ".img.gz",
    ".hdr.gz",
    ".BRIK.gz",
    ".1D.dset",
    ".niml.dset",
    ".nii",
    ".hdr",
    ".img",
    ".HEAD",
    ".BRIK",
    ".mha",
    ".mhd",
    ".nrrd",
    ".nhdr",
    ".h5",
    ".hdf5",
    ".mat",
    ".txt",
    ".csv",
    ".json",
    ".1D",
    ".mgz",
    ".mgh",
    ".gii",
    ".tfm",
    ".xfm",
    ".vtk",
    ".par",
)
_AFNI_VIEW = re.compile(r"(\+(?:orig|tlrc|acpc)(?:\.(?:HEAD|BRIK)(?:\.gz)?)?)$")


def suffix(name: str) -> str:
    """The part of a file name a program reads its format from: ``.nii.gz``, ``+orig.HEAD``,
    ``.mat``; else the last suffix."""
    if m := _AFNI_VIEW.search(name):
        return m.group(1)
    lower = name.lower()
    for s in SUFFIXES:
        if lower.endswith(s.lower()) and len(name) > len(s):
            return name[-len(s) :]
    return Path(name).suffix


def companions(path: Path) -> list[Path]:
    """The files a program reads beside ``path`` without being told: an Analyze or NIfTI pair's
    other half, an AFNI dataset's ``.BRIK`` (``.gz``)."""
    name = path.name
    stems = []
    for main, others in (
        (".hdr", (".img", ".img.gz")),
        (".hdr.gz", (".img.gz", ".img")),
        (".img", (".hdr",)),
        (".img.gz", (".hdr.gz", ".hdr")),
        (".HEAD", (".BRIK", ".BRIK.gz")),
    ):
        if name.endswith(main):
            stems += [path.with_name(name[: -len(main)] + o) for o in others]
    return [p for p in stems if p.is_file()]


class Uncacheable(Exception):
    """A run the cache cannot key or store faithfully (the reason is the message)."""


def temp_root() -> Path:
    return Path(tempfile.gettempdir()).resolve()


def under(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


# ------------------------------------------------------------------------------------------------
# The oracle's identity


@functools.cache
def _shared_libraries(binary: str, ld_library_path: str) -> tuple[tuple[str, str], ...]:
    """``(soname, SHA-256)`` of every shared library ``binary`` loads (``ldd``)."""
    env = {k: v for k, v in os.environ.items() if k not in ("LD_PRELOAD", "LD_LIBRARY_PATH")}
    if ld_library_path:
        env["LD_LIBRARY_PATH"] = ld_library_path
    ldd = shutil.which("ldd")
    if ldd is None:
        return ()
    p = subprocess.run([ldd, binary], capture_output=True, text=True, env=env, check=False)
    libs = []
    for line in p.stdout.splitlines():
        line = line.strip()
        m = re.match(r"(\S+) => (\S+) \(0x[0-9a-f]+\)", line) or re.match(
            r"(/\S+) \(0x[0-9a-f]+\)", line
        )
        if not m:
            continue
        name, path = (m.group(1), m.group(2)) if m.lastindex == 2 else (m.group(1), m.group(1))
        target = Path(path)
        if target.is_file():
            libs.append((Path(name).name, sha256_file(target.resolve())))
    return tuple(sorted(libs))


def program_identity(binary: Path, env: dict[str, str]) -> dict:
    """The oracle's identity: the program's SHA-256, its shared libraries' (and, for a script,
    every file of its build directory)."""
    resolved = binary.resolve()
    identity: dict = {"program": binary.name, "sha256": sha256_file(resolved)}
    with resolved.open("rb") as f:
        script = f.read(2) == b"#!"
    if script:
        build = {
            p.name: sha256_file(p)
            for p in sorted(resolved.parent.iterdir())
            if p.is_file() and p != resolved
        }
        identity["build"] = hashlib.sha256(json.dumps(build, sort_keys=True).encode()).hexdigest()
    else:
        libs = _shared_libraries(str(resolved), env.get("LD_LIBRARY_PATH", ""))
        identity["libraries"] = dict(libs)
    return identity


@functools.cache
def apptainer_version() -> str:
    exe = shutil.which("apptainer")
    if exe is None:
        return "not found"
    p = subprocess.run([exe, "--version"], capture_output=True, text=True, check=False)
    return p.stdout.strip()


def image_identity(image: Path, program: str) -> dict:
    return {
        "image": image.name,
        "image_sha256": sha256_file(image.resolve()),
        "program": program,
        "apptainer": apptainer_version(),
    }


@functools.cache
def machine() -> dict:
    """The architecture and the CPU (model and flags: glibc chooses its kernels by them)."""
    model, flags = platform.processor() or platform.machine(), ""
    try:
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.startswith("model name") and model == (
                platform.processor() or platform.machine()
            ):
                model = line.split(":", 1)[1].strip()
            if line.startswith("flags") and not flags:
                flags = " ".join(sorted(line.split(":", 1)[1].split()))
    except OSError:
        pass
    return {
        "arch": platform.machine(),
        "cpu": model,
        "flags": hashlib.sha256(flags.encode()).hexdigest()[:16],
    }


# ------------------------------------------------------------------------------------------------
# Paths in the arguments


@dataclass(frozen=True)
class Input:
    """A file the run reads: where it is, its bytes' SHA-256 (its blob), its suffix, the files
    read beside it (``(suffix, SHA-256, path)``), and its content's identity in the key
    (:func:`~larmorx_validation.oracle_cache.hashing.sha256_content`)."""

    path: Path
    sha256: str
    suffix: str
    companions: tuple[tuple[str, str, Path], ...]
    content: str
    companions_content: tuple[str, ...] = ()
    #: the argument as written when it named the file relative to the working directory
    relative: str | None = None


@dataclass
class Prepared:
    """A run made path-independent: the key's parts, and how the run's paths map to them."""

    parts: dict
    #: (placeholder name, absolute path) of every place, in order.
    places: list[tuple[str, Path]]
    #: the input files, in order (``{in0}``, ``{in1}``, ...)
    inputs: list[Input]
    #: the arguments with placeholders (``{in0}``, ``{out0}/x.nii.gz``)
    template: list[str]
    #: places watched for outputs: name -> (path, recursive)
    watched: dict[str, tuple[Path, bool]] = field(default_factory=dict)
    #: the places that are files (a declared output file), not directories
    files: set[str] = field(default_factory=set)
    #: the environment's variables that matter, as ``verify`` sets them again: paths as
    #: placeholders, ``LD_PRELOAD`` as written, ``HOME`` as ``{home}``
    rerun_env: dict[str, str] = field(default_factory=dict)
    #: ``HOME`` when it is in the key (a small directory of the run's, AFNI's): its path in the
    #: messages is replaced like a place's
    home: Path | None = None

    def role_of(self, path: Path) -> str | None:
        """``{place}/rest`` of a path inside a directory place (the longest), or ``None``."""
        best = None
        for name, p in self.places:
            if name in self.files:
                continue
            if (path == p or under(path, p)) and (best is None or len(str(p)) > len(str(best[1]))):
                best = (name, p)
        if best is None:
            return None
        rest = path.relative_to(best[1]).as_posix()
        return "{" + best[0] + "}" + ("" if rest == "." else "/" + rest)

    def watch_roles(self) -> list[tuple[str, bool]]:
        """The watched directories as ``verify`` finds them again: inside a place by its role,
        elsewhere as written."""
        return [(self.role_of(path) or str(path), r) for path, r in self.watched.values()]

    @functools.cached_property
    def key(self) -> str:
        blob = json.dumps(self.parts, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        return hashlib.sha256(blob.encode()).hexdigest()


class _Mapper:
    """Assigns placeholders to the paths of one run."""

    def __init__(self, cwd: Path | None, outputs: Sequence[Path]):
        self.places: list[tuple[str, Path]] = []
        self.inputs: list[Input] = []
        self._input_index: dict[Path, int] = {}
        self.watched: dict[str, tuple[Path, bool]] = {}
        self.files: set[str] = set()
        self.dirs: set[str] = set()
        self.home: Path | None = None
        self.cwd = cwd
        self.tmp = temp_root()
        if cwd is not None:
            self._place("cwd", cwd, recursive=True)
        else:
            # The process's own directory: a file the run writes there (a relative name) makes
            # it impossible to store.
            self.watched["w-cwd"] = (Path.cwd(), False)
        for i, out in enumerate(outputs):
            out = Path(out).absolute()
            if out.is_dir():
                self._place(f"out{i}", out, recursive=True)
            else:
                # a file the run writes: the file is the place; its directory is watched too
                # (a place of its own, so that whatever else the run writes there is named)
                self._place(f"out{i}", out, recursive=False, file=True)
                self._place(f"out{i}p", out.parent, recursive=False)

    def _place(self, name: str, path: Path, *, recursive: bool, file: bool = False) -> str:
        path = Path(os.path.abspath(path))
        for existing, p in self.places:
            if p == path:
                return existing
        self.places.append((name, path))
        if file:
            self.files.add(name)
        watch = path.parent if file else path
        if not any(w == watch and r for w, r in self.watched.values()):
            self.watched[name] = (watch, recursive)
        return name

    def _input(self, path: Path) -> str:
        path = Path(os.path.abspath(path))
        if path not in self._input_index:
            if under(path, self.tmp) and path.parent != self.tmp:
                # the run may write it (or beside it): its directory is a place, watched
                self._place(f"d{sum(n.startswith('d') for n, _ in self.places)}", path.parent,
                            recursive=False)  # fmt: skip
            comps = tuple((suffix(c.name), sha256_file(c), c) for c in companions(path))
            self._input_index[path] = len(self.inputs)
            self.inputs.append(
                Input(
                    path,
                    sha256_file(path),
                    suffix(path.name),
                    comps,
                    sha256_content(path),
                    tuple(sha256_content(c) for _s, _h, c in comps),
                )
            )
        return f"in{self._input_index[path]}"

    def env_value(self, value: str) -> tuple[str, str]:
        """A variable's value in the key (an existing file by its content, a path inside a
        place by its role) and as ``verify`` sets it again (files as written)."""
        pieces = _SPLIT.split(value)
        key, rerun = [], []
        for i, piece in enumerate(pieces):
            if i % 2 or not piece.startswith("/"):
                key.append(piece)
                rerun.append(piece)
                continue
            path = Path(os.path.abspath(piece))
            if path.is_file():
                key.append(f"<{path.name} {sha256_content(path)}>")
                rerun.append(piece)
            elif (role := self._role(path)) is not None:
                key.append(role)
                rerun.append(role)
            else:
                key.append(piece)
                rerun.append(piece)
        return "".join(key), "".join(rerun)

    def _role(self, path: Path) -> str | None:
        """``{place}/rest`` for a path inside a place (the longest place first)."""
        best = None
        for name, p in self.places:
            if (path == p or under(path, p)) and (best is None or len(str(p)) > len(str(best[1]))):
                best = (name, p)
        if best is None:
            return None
        rest = path.relative_to(best[1]).as_posix()
        return "{" + best[0] + "}" + ("" if rest == "." else "/" + rest)

    def _watch(self, directory: Path) -> None:
        """Watch ``directory`` for the files the run writes (unless a watch covers it)."""
        for w, recursive in self.watched.values():
            if w == directory or (recursive and under(directory, w)):
                return
        self.watched[f"w{len(self.watched)}"] = (directory, False)

    def _note_directories(self, target: Path, anchor: Path) -> None:
        """The directories that exist between ``target``'s place and ``target`` (``verify``
        makes them again; they are in the key: a run writing into a missing directory is not
        one writing into a directory that exists)."""
        best = None
        for name, p in self.places:
            if (anchor == p or under(anchor, p)) and (
                best is None or len(str(p)) > len(str(best[1]))
            ):
                best = (name, p)
        if best is None:
            return
        d = anchor
        while d != best[1] and under(d, best[1]):
            self.dirs.add("{" + best[0] + "}/" + d.relative_to(best[1]).as_posix())
            d = d.parent

    def token(self, text: str) -> str:
        """One path-like piece of an argument, with its path replaced."""
        if not text:
            return text
        relative = not text.startswith("/")
        if not relative:
            path = Path(text)
        elif self.cwd is not None or os.sep in text or Path(text).suffix:
            path = (self.cwd or Path.cwd()) / text
            if not path.is_file() and self.cwd is None:
                return text  # a relative name, no working directory: kept as written
        else:
            return text
        if path.is_file():
            name = self._input(path)
            if relative:
                i = int(name[2:])
                self.inputs[i] = replace(self.inputs[i], relative=text)
            return "{" + name + "}"
        # A directory the run may write in, or a path it may create.
        target = Path(os.path.abspath(path))
        anchor = target if target.is_dir() else _existing_ancestor(target)
        role = self._role(target)
        temporary = anchor is not None and under(anchor, self.tmp) and anchor != self.tmp
        if role is None and not relative and temporary:
            # a temporary directory: a place of its own
            n = sum(name.startswith("d") for name, _ in self.places)
            self._place(f"d{n}", anchor, recursive=False)
            role = self._role(target)
        if role is not None and anchor is not None:
            self._watch(anchor)
            self._note_directories(target, anchor)
            return text if relative else role
        if not relative and anchor is not None and len(anchor.parts) > 3:
            # Elsewhere: kept as written, but watched: a file the run writes there makes it
            # impossible to store (the cache could not put it back).
            self._watch(anchor)
        return text

    def argument(self, arg: str) -> str:
        pieces = _SPLIT.split(arg)
        return "".join(p if i % 2 else self.token(p) for i, p in enumerate(pieces))


def _existing_ancestor(path: Path) -> Path | None:
    for parent in path.parents:
        if parent.is_dir():
            return parent
    return None


@functools.cache
def _real_home() -> str:
    try:
        import pwd

        return pwd.getpwuid(os.getuid()).pw_dir
    except (ImportError, KeyError):  # pragma: no cover - not POSIX
        return str(Path.home())


def _env_part(
    env: dict[str, str], mapper: _Mapper, family: str | None
) -> tuple[dict[str, str], dict[str, str]]:
    """The environment's part of the key (the variables a program of ``family`` reads), and the
    same variables as ``verify`` sets them."""
    out, rerun = {}, {}
    for name in sorted(env):
        if not env_matters(name, family, env):
            continue
        value = env[name]
        if name == "HOME":
            home = Path(value)
            if os.path.abspath(value) == os.path.abspath(_real_home()) or not home.is_dir():
                continue  # the user's home: not part of the key (AFNI's runs set an empty one)
            digest = directory_digest(home, ignore=WRITTEN_IN_HOME)
            if digest is None:
                raise Uncacheable(f"HOME ({value}) is a large directory, not hashed")
            out[name] = f"<directory {digest}>"
            rerun[name] = "{home}"
            mapper.home = Path(os.path.abspath(value))
        else:
            out[name], rerun[name] = mapper.env_value(value)
    return out, rerun


def prepare(
    oracle: dict,
    args: Sequence[str],
    env: dict[str, str],
    *,
    cwd: Path | None = None,
    outputs: Sequence[Path] = (),
    threads_known: bool = True,
    family: str | None = None,
    extra: dict | None = None,
) -> Prepared:
    """The key's parts of running the oracle ``oracle`` (``program_identity`` or
    ``image_identity``) with ``args`` in ``env``; ``family`` names the variables that matter
    (:data:`ENV_FAMILIES`; ``None``: every family's)."""
    mapper = _Mapper(Path(os.path.abspath(cwd)) if cwd is not None else None, outputs)
    template = [mapper.argument(str(a)) for a in args]
    env_part, rerun_env = _env_part(env, mapper, family)
    parts = {
        "format": KEY_FORMAT,
        "oracle": oracle,
        "args": template,
        "inputs": [
            {
                "content": i.content,
                "suffix": i.suffix,
                **(
                    {
                        "companions": dict(
                            zip((c[0] for c in i.companions), i.companions_content, strict=True)
                        )
                    }
                    if i.companions
                    else {}
                ),
            }
            for i in mapper.inputs
        ],
        "env": env_part,
        "cwd": "{cwd}" if cwd is not None else None,
        "directories": sorted(mapper.dirs),
        "machine": machine() | ({} if threads_known else {"cpus": _cpus()}),
    }
    if extra:
        parts["extra"] = extra
    return Prepared(
        parts,
        mapper.places,
        mapper.inputs,
        template,
        mapper.watched,
        mapper.files,
        rerun_env,
        mapper.home,
    )


def _cpus() -> int:
    try:
        return len(os.sched_getaffinity(0))
    except AttributeError:  # pragma: no cover - not Linux
        return os.cpu_count() or 1


__all__ = [
    "COMMON_ENV",
    "ENV_FAMILIES",
    "KEY_FORMAT",
    "Input",
    "Prepared",
    "Uncacheable",
    "apptainer_version",
    "companions",
    "env_matters",
    "env_names",
    "image_identity",
    "machine",
    "prepare",
    "program_identity",
    "suffix",
    "temp_root",
]
