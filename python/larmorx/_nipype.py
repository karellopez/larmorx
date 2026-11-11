# SPDX-License-Identifier: Apache-2.0
"""nipype's command lines, as nipype 1.12.0 writes them, and their runs through larmorx.

sdcflows, niworkflows and fMRIPrep run ANTs', FSL's and AFNI's programs through nipype's
interfaces: nipype formats each interface's inputs into a command line (``command.txt`` in the
node's directory), runs it through the shell in the node's directory and lists the outputs it
expects. This module is that formatting, ported from nipype 1.12.0 (Apache-2.0, the nipype
developers; the version of every environment larmorx is checked in, fMRIPrep 25.2.6's image
included): ``nipype/interfaces/base/core.py`` (``CommandLine._parse_inputs``, ``_format_arg``,
``_filename_from_source``, ``_list_outputs``), ``nipype/utils/filemanip.py``
(:func:`split_filename`, :func:`fname_presuffix`) and the interfaces ``ants.Registration``
(with niworkflows 1.15.4's ``FixHeaderRegistration``: ``restrict_deformation``),
``ants.ApplyTransforms`` (niworkflows' ``FixHeaderApplyTransforms``: ``identity``),
``ants.N4BiasFieldCorrection``, ``ants.ImageMath``, ``ants.DenoiseImage``, ``fsl.BET``,
``fsl.MultiImageMaths``, ``fsl.BinaryMaths``, ``fsl.TOPUP``, ``afni.Automask`` and ``afni.Volreg``: every input with
a command-line form, in nipype's order (positions first, then by name, negative positions last),
with nipype's file names and quoting.

A :class:`Command` holds the program, nipype's arguments (``cmdline`` is the line nipype writes)
and nipype's outputs; :func:`run` runs it in its node directory through larmorx's port of the
program (``argv``: the words the shell gives the program). The interfaces' settings files
(sdcflows' registration JSON files) are read as nipype's ``from_file`` reads them, with the
values converted as nipype's input traits convert them (a JSON ``6`` given for a float stage
setting is written ``6.0``).

Not ported (nothing on these paths reaches them): nipype's version checks of inputs (``min_ver``,
``max_ver``: ``fixed_image_mask``, ``moving_image_mask`` belong to ANTs <= 2.1.0 and raise here),
and its ``xor``/``requires`` checks beyond the ones that change the line.
``antsApplyTransforms`` runs through larmorx's port of the program on files (its transform
files read as the program reads them), as nipype runs it.
"""

# nipype formats with Python's % operator (``%g``, ``%d`` of floats and booleans), which this
# port keeps where nipype uses it: format specifiers would not give the same text.
# ruff: noqa: UP031

from __future__ import annotations

import ntpath
import os
import shlex
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any, ClassVar

__all__ = [
    "UNDEFINED",
    "Command",
    "apply_transforms",
    "automask",
    "bet",
    "binary_maths",
    "copyfile",
    "denoise_image",
    "ensure_list",
    "fname_presuffix",
    "get_related_files",
    "image_math",
    "load_settings",
    "multi_image_maths",
    "n4_bias_field_correction",
    "registration",
    "run",
    "split_filename",
    "topup",
    "volreg",
]


class _Undefined:
    """nipype's ``Undefined``: false, of length 0."""

    _instance: _Undefined | None = None

    def __new__(cls) -> _Undefined:
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __bool__(self) -> bool:
        return False

    def __len__(self) -> int:
        return 0

    def __repr__(self) -> str:
        return "<undefined>"


#: An input left undefined (nipype's ``Undefined``).
UNDEFINED: Any = _Undefined()


class TraitError(Exception):
    """traits' ``TraitError``: an interface's input or output of the wrong type (nipype
    validates an interface's outputs against its output spec)."""


def trait_error(spec: str, name: str, info: str, value: Any) -> TraitError:
    """traits' message for ``value`` set on the trait ``name`` of a ``spec`` instance."""
    return TraitError(
        f"The '{name}' trait of a {spec} instance must be {info}, but a value of {value!r} "
        f"{type(value)!r} was specified."
    )


def isdefined(value: Any) -> bool:
    return value is not UNDEFINED


# ------------------------------------------------------------------------------------------
# nipype/utils/filemanip.py, nipype/utils/misc.py


def split_filename(fname: str | os.PathLike[str]) -> tuple[str, str, str]:
    """nipype's ``split_filename``: directory, stem and extension (``.nii.gz``, ``.tar.gz`` and
    ``.niml.dset`` kept whole)."""
    fname = os.fspath(fname)
    pth = os.path.dirname(fname)
    base = os.path.basename(fname)
    for special in (".nii.gz", ".tar.gz", ".niml.dset"):
        n = len(special)
        if len(base) > n and base[-n:].lower() == special.lower():
            return pth, base[:-n], base[-n:]
    stem, ext = os.path.splitext(base)
    return pth, stem, ext


def fname_presuffix(
    fname: str | os.PathLike[str],
    prefix: str = "",
    suffix: str = "",
    newpath: str | os.PathLike[str] | None = None,
    use_ext: bool = True,
) -> str:
    """nipype's ``fname_presuffix``: ``fname`` with ``prefix`` and ``suffix`` around its stem,
    in ``newpath`` (made absolute) when given, else in its own directory."""
    pth, stem, ext = split_filename(fname)
    if not use_ext:
        ext = ""
    if newpath:
        pth = os.path.abspath(os.fspath(newpath))
    return os.path.join(pth, prefix + stem + suffix + ext)


def ensure_list(value: Any) -> list[Any] | None:
    """nipype's ``ensure_list``: a string as a one-item list, a list or tuple as a list."""
    if value is None:
        return None
    if isinstance(value, (str, bytes)):
        return [value]
    if isinstance(value, (list, tuple)):
        return list(value)
    return [value]


#: nipype's ``related_filetype_sets``: files that travel together.
RELATED_FILETYPE_SETS = ((".hdr", ".img", ".mat"), (".nii", ".mat"), (".BRIK", ".HEAD"))


def get_related_files(filename: str, include_this_file: bool = True) -> list[str]:
    """nipype's ``get_related_files``: the NIfTI pair, SPM ``.mat`` and AFNI ``.HEAD`` files of
    ``filename`` (``[filename]`` itself when it has none)."""
    path, name, this_type = split_filename(filename)
    related = [
        os.path.join(path, f"{name}{related_type}")
        for type_set in RELATED_FILETYPE_SETS
        if this_type in type_set
        for related_type in type_set
        if include_this_file or related_type != this_type
    ]
    return related or [filename]


def copyfile(
    originalfile: str | os.PathLike[str],
    newfile: str | os.PathLike[str],
    *,
    use_hardlink: bool = True,
    copy_related_files: bool = True,
) -> str:
    """nipype's ``copyfile(originalfile, newfile, copy=True, use_hardlink=use_hardlink)`` with
    the content hash: an existing ``newfile`` that is the same file or has the same content is
    kept, else replaced; a hard link to the original's real path, else a copy; then the related
    files that exist (:func:`get_related_files`) the same way. (niworkflows' sink removes
    ``newfile`` first, so the comparison of an existing file never decides its result.)"""
    import hashlib
    import shutil

    originalfile, newfile = os.fspath(originalfile), os.fspath(newfile)

    def content_hash(p: str) -> str:
        h = hashlib.md5()
        with open(p, "rb") as f:
            for chunk in iter(lambda: f.read(8192), b""):
                h.update(chunk)
        return h.hexdigest()

    keep = False
    if os.path.lexists(newfile):
        if os.path.islink(newfile):
            keep = False  # copy=True never keeps a link
        elif os.path.samefile(newfile, originalfile):
            keep = True
        else:
            keep = content_hash(newfile) == content_hash(originalfile)
        if not keep:
            os.unlink(newfile)
    if not keep and use_hardlink:
        try:
            os.link(os.path.realpath(originalfile), newfile)
        except OSError:
            use_hardlink = False
        else:
            keep = True
    if not keep:
        with suppress_shutil_error():
            shutil.copyfile(originalfile, newfile)
    if copy_related_files:
        pairs = zip(
            get_related_files(originalfile, include_this_file=False),
            get_related_files(newfile, include_this_file=False),
            strict=False,
        )
        for alt_ofile, alt_nfile in pairs:
            if os.path.exists(alt_ofile):
                copyfile(alt_ofile, alt_nfile, use_hardlink=use_hardlink, copy_related_files=False)
    return newfile


class suppress_shutil_error:
    """nipype's ``except shutil.Error as e: fmlogger.warning(str(e))``."""

    def __enter__(self) -> None:
        return None

    def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> bool:
        import logging
        import shutil

        if exc_type is not None and issubclass(exc_type, shutil.Error):
            logging.getLogger("larmorx.nipype").warning(str(exc))
            return True
        return False


# ------------------------------------------------------------------------------------------
# the command line (nipype/interfaces/base/core.py)


@dataclass(frozen=True)
class Command:
    """A command line as nipype builds it.

    ``args`` are nipype's formatted arguments in order (one may hold several words, or be empty:
    nipype then writes two spaces); :attr:`cmdline` is the line nipype writes into
    ``command.txt``; :meth:`argv` the words the shell gives the program. ``outputs`` are the
    interface's outputs as nipype lists them (absolute paths in ``cwd``), ``environ`` the
    variables nipype sets for the run (``NSLOTS``, ``FSLOUTPUTTYPE``)."""

    program: str
    args: tuple[str, ...]
    cwd: str
    outputs: Mapping[str, Any] = field(default_factory=dict)
    environ: Mapping[str, str] = field(default_factory=dict)
    #: Files the interface writes before the program runs (TOPUP's encoding file):
    #: ``{path: text}``.
    files: Mapping[str, str] = field(default_factory=dict)
    #: ``args`` with the output names nipype gives relative to the node directory made
    #: absolute (for programs that run in this process, whose directory is not the node's);
    #: ``None``: the same as ``args``.
    run_args: tuple[str, ...] | None = None

    @property
    def cmdline(self) -> str:
        return " ".join([self.program, *self.args])

    def argv(self) -> list[str]:
        """The program's arguments as the shell splits nipype's line (``shlex``: words, quotes
        removed; on Windows a backslash is a path's separator, :func:`_split_line`)."""
        return _split_line(" ".join(self.args))

    def run_argv(self) -> list[str]:
        """:meth:`argv` with the node directory's output names absolute."""
        return _split_line(" ".join(self.args if self.run_args is None else self.run_args))


def _split_line(line: str, windows: bool = os.name == "nt") -> list[str]:
    """The words of nipype's command line as a POSIX shell splits it (``shlex.split``: nipype
    quotes paths with ``shlex.quote``). On Windows a backslash separates a path's folders, so it
    is no escape there: the same words, quotes removed, backslashes kept."""
    if not windows:
        return shlex.split(line)
    lex = shlex.shlex(line, posix=True)
    lex.whitespace_split = True
    lex.commenters = ""
    lex.escape = ""
    return list(lex)


def _bet_relpath(path: str, start: str, windows: bool = os.name == "nt") -> str:
    """nipype's BET makes its input and output names relative to the node's folder
    (``os.path.relpath``: BET fails with long paths). On Windows nipype's quotes around a path
    (``shlex.quote`` quotes a backslash) are kept around the relative path, and a path on
    another drive than the folder stays as it is (no relative path joins two drives)."""
    if not windows:
        return os.path.relpath(path, start=start)
    quoted = len(path) > 1 and path[0] == path[-1] == "'"
    inner = path[1:-1] if quoted else path
    try:
        rel = ntpath.relpath(inner, start=start)
    except ValueError:
        rel = inner
    return shlex.quote(rel) if quoted else rel


@dataclass(frozen=True)
class _Trait:
    """An input with a command-line form (an ``argstr``), as nipype's input spec declares it.

    ``kind``: ``"bool"`` (a ``Bool`` whose ``argstr`` has no ``%``: the flag when true),
    ``"path"`` (a file name: quoted for the shell), ``"list"`` (joined by ``sep``),
    ``"compound"`` (an ``Either``: a list value joined, else as is) or ``"value"``."""

    name: str
    argstr: str
    kind: str = "value"
    position: int | None = None
    sep: str | None = None
    default: Any = UNDEFINED
    name_source: str | None = None
    name_template: str | None = None
    keep_extension: bool = False
    genfile: bool = False
    xor: tuple[str, ...] = ()
    requires: tuple[str, ...] = ()


class _Interface:
    """A nipype command-line interface: its traits with a command-line form, the defaults of
    its other inputs, and nipype's ``_parse_inputs`` over them."""

    program = ""
    traits: ClassVar[tuple[_Trait, ...]] = ()
    defaults: ClassVar[dict[str, Any]] = {}

    def __init__(self, inputs: Mapping[str, Any], cwd: str | os.PathLike[str]) -> None:
        names = {t.name for t in self.traits} | set(self.defaults)
        unknown = sorted(set(inputs) - names - set(self.extra_inputs()))
        if unknown:
            raise TypeError(f"{type(self).__name__[1:]}: unknown inputs {unknown}")
        values: dict[str, Any] = {t.name: t.default for t in self.traits}
        values.update(self.defaults)
        values.update({k: v for k, v in inputs.items() if v is not None})
        self.inputs = values
        self.cwd = os.path.abspath(os.fspath(cwd))
        self._trait = {t.name: t for t in self.traits}
        #: Whether output names are written absolute (:attr:`Command.run_args`).
        self.absolute = False

    def in_cwd(self, name: Any) -> Any:
        """An output name as the command line gives it: relative as nipype writes it, or in the
        node directory when :attr:`absolute`."""
        if self.absolute and name and isdefined(name) and not os.path.isabs(os.fspath(name)):
            return os.path.join(self.cwd, os.fspath(name))
        return name

    def reset(self) -> None:
        """Clear the formatting state before the line is formatted again."""

    def extra_inputs(self) -> tuple[str, ...]:
        return ()

    def get(self, name: str) -> Any:
        return self.inputs.get(name, UNDEFINED)

    def defined(self, name: str) -> bool:
        return isdefined(self.get(name))

    def abspath(self, name: str) -> str:
        """``os.path.abspath`` in the node directory (the cwd nipype runs in)."""
        return os.path.normpath(os.path.join(self.cwd, os.fspath(name)))

    # nipype's base formatting
    def format_arg(self, t: _Trait, value: Any) -> str | None:
        return _format_default(t, value)

    def gen_filename(self, name: str) -> Any:
        raise NotImplementedError(name)

    def overload_extension(self, value: str, name: str) -> str:
        return value

    def filename_from_source(self, name: str, chain: list[str] | None = None) -> Any:
        if chain is None:
            chain = []
        t = self._trait.get(name)
        retval = self.get(name)
        if isdefined(retval) and "%s" not in str(retval):
            return retval
        if t is None or not t.name_source:
            return retval
        if any(self.defined(f) for f in t.xor):
            return retval
        if not all(self.defined(f) for f in t.requires):
            return retval
        template = retval if isdefined(retval) and "%s" in str(retval) else t.name_template
        if not template:
            template = "%s_generated"
        ns = t.name_source
        source_ext = None
        if self.defined(ns):
            source = self.get(ns)
            while isinstance(source, (list, tuple)):
                source = source[0]
            try:
                _, base, source_ext = split_filename(source)
            except (AttributeError, TypeError):
                base = source
        else:
            if name in chain:
                raise RuntimeError("Mutually pointing name_sources")
            chain.append(name)
            base = self.filename_from_source(ns, chain)
            if not isdefined(base):
                return retval
            _, _, source_ext = split_filename(base)
        retval = template % base
        _, _, ext = split_filename(retval)
        if t.keep_extension and (ext or source_ext):
            if not ext and source_ext:
                retval = retval + source_ext
        else:
            retval = self.overload_extension(retval, name)
        return retval

    def parse_inputs(self, skip: Sequence[str] = ()) -> list[str]:
        all_args: list[str] = []
        initial: dict[int, str] = {}
        final: dict[int, str] = {}
        for t in sorted(self.traits, key=lambda t: t.name):
            if t.name in skip:
                continue
            value = self.get(t.name)
            if t.name_source:
                value = self.in_cwd(self.filename_from_source(t.name))
            elif t.genfile and (not isdefined(value) or value is None):
                value = self.gen_filename(t.name)
            if not isdefined(value):
                continue
            arg = self.format_arg(t, value)
            if arg is None:
                continue
            if t.position is not None:
                (initial if t.position >= 0 else final)[t.position] = arg
            else:
                all_args.append(arg)
        return (
            [v for _, v in sorted(initial.items())]
            + all_args
            + [v for _, v in sorted(final.items())]
        )

    def list_outputs(self) -> dict[str, Any]:
        """nipype's ``CommandLine._list_outputs``: every ``name_source`` input's file name."""
        out: dict[str, Any] = {}
        for t in self.traits:
            if t.name_source:
                fname = self.filename_from_source(t.name)
                if isdefined(fname):
                    out[t.name] = self.abspath(fname)
        return out

    def environ(self) -> dict[str, str]:
        return {}

    def files(self) -> dict[str, str]:
        return {}

    def command(self) -> Command:
        args = tuple(self.parse_inputs())
        self.reset()
        self.absolute = True
        run_args = tuple(self.parse_inputs())
        self.absolute = False
        self.reset()
        return Command(
            self.program,
            args,
            self.cwd,
            self.list_outputs(),
            self.environ(),
            self.files(),
            None if run_args == args else run_args,
        )


def _format_default(t: _Trait, value: Any) -> str | None:
    """nipype's ``CommandLine._format_arg``."""
    if t.kind == "bool":
        return t.argstr if value else None
    if t.kind == "list" or (t.kind == "compound" and isinstance(value, list)):
        sep = t.sep if t.sep is not None else " "
        if t.argstr.endswith("..."):
            argstr = t.argstr[:-3]
            return sep.join(argstr % elt for elt in value)
        return t.argstr % sep.join(str(elt) for elt in value)
    if t.kind == "path" and "'%s'" not in t.argstr and '"%s"' not in t.argstr:
        value = shlex.quote(os.fspath(value))
    return t.argstr % (value,) if not isinstance(value, tuple) else t.argstr % value


# ------------------------------------------------------------------------------------------
# the traits' conversions (traits' Float, Int, Range, Tuple, Either)


def _float(v: Any) -> float:
    if isinstance(v, bool) or not isinstance(v, (int, float)):
        raise TypeError(f"expected a number, got {v!r}")
    return float(v)


def _is_int(v: Any) -> bool:
    return isinstance(v, int) and not isinstance(v, bool)


#: ``Registration.transform_parameters``' tuple forms, in nipype's ``Either`` order (``F``: a
#: float, which takes an integer and makes it a float; ``I``: an integer only).
_TRANSFORM_PARAMETER_FORMS = ("F", "FFF", "FIII", "FIFFFF", "FFFI", "FIIII")


def _transform_parameters(value: Any) -> tuple[Any, ...]:
    items = list(value)
    for form in _TRANSFORM_PARAMETER_FORMS:
        if len(form) != len(items):
            continue
        out: list[Any] = []
        for kind, v in zip(form, items, strict=True):
            if kind == "I":
                if not _is_int(v):
                    break
                out.append(v)
            else:
                if isinstance(v, bool) or not isinstance(v, (int, float)):
                    break
                out.append(float(v))
        else:
            return tuple(out)
    raise TypeError(f"transform_parameters: no tuple form takes {value!r}")


def _interpolation_parameters(value: Any) -> tuple[Any, ...]:
    """``Either(Tuple(Int), Tuple(Float, Float), Tuple(Str))``."""
    items = list(value) if isinstance(value, (list, tuple)) else [value]
    if len(items) == 1 and _is_int(items[0]):
        return (items[0],)
    if len(items) == 2 and all(isinstance(v, (int, float)) for v in items):
        return (float(items[0]), float(items[1]))
    if len(items) == 1 and isinstance(items[0], str):
        return (items[0],)
    raise TypeError(f"interpolation_parameters: no tuple form takes {value!r}")


def _floats2(value: Any) -> list[list[float]]:
    return [[_float(x) for x in stage] for stage in value]


def _floats_or_lists(value: Any) -> list[Any]:
    """A per-stage float setting (``metric_weight``, ``sampling_percentage``): a number, a list
    of numbers (several metrics) or ``None``."""
    out: list[Any] = []
    for stage in value:
        if isinstance(stage, (list, tuple)):
            out.append([None if v is None else _float(v) for v in stage])
        else:
            out.append(None if stage is None else _float(stage))
    return out


def load_settings(path: str | os.PathLike[str]) -> dict[str, Any]:
    """A nipype settings file (``Registration(from_file=...)``): its JSON, as nipype's
    ``load_inputs_from_json`` reads it."""
    import json

    with open(os.fspath(path), encoding="utf-8") as f:
        return dict(json.load(f))


# ------------------------------------------------------------------------------------------
# ANTs (nipype/interfaces/ants)


class _AntsCommand(_Interface):
    def environ(self) -> dict[str, str]:
        n = self.get("num_threads")
        if not isdefined(n):
            n = 1
        return {} if n == -1 else {"NSLOTS": str(n)}


def _xarray(values: Sequence[Any]) -> str:
    """``ANTSCommand._format_xarray``: ``1x2x3``."""
    return "x".join(str(x) for x in values)


class _Registration(_AntsCommand):
    """``ants.Registration`` (niworkflows' ``FixHeaderRegistration``)."""

    program = "antsRegistration"
    traits = (
        _Trait("args", "%s"),
        _Trait("collapse_output_transforms", "--collapse-output-transforms %d", default=True),
        _Trait("dimension", "--dimensionality %d", default=3),
        _Trait("fixed_image_mask", "%s", "path", xor=("fixed_image_masks",)),
        _Trait("float", "--float %d"),
        _Trait("initial_moving_transform", "%s", xor=("initial_moving_transform_com",)),
        _Trait("initial_moving_transform_com", "%s", xor=("initial_moving_transform",)),
        _Trait(
            "initialize_transforms_per_stage", "--initialize-transforms-per-stage %d", default=False
        ),
        _Trait("interpolation", "%s", default="Linear"),
        _Trait("output_transform_prefix", "%s", default="transform"),
        _Trait("random_seed", "--random-seed %d"),
        _Trait("restore_state", "--restore-state %s", "path"),
        _Trait("save_state", "--save-state %s", "path"),
        _Trait("transforms", "%s"),
        _Trait("verbose", "-v", "bool", default=False),
        _Trait("winsorize_lower_quantile", "%s", default=0.0),
        _Trait("winsorize_upper_quantile", "%s", default=1.0),
        _Trait("write_composite_transform", "--write-composite-transform %d", default=False),
    )
    defaults: ClassVar[dict[str, Any]] = {
        "fixed_image": UNDEFINED,
        "fixed_image_masks": UNDEFINED,
        "moving_image": UNDEFINED,
        "moving_image_mask": UNDEFINED,
        "moving_image_masks": UNDEFINED,
        "invert_initial_moving_transform": UNDEFINED,
        "metric": UNDEFINED,
        "metric_weight": [1.0],
        "radius_or_number_of_bins": [5],
        "sampling_strategy": UNDEFINED,
        "sampling_percentage": UNDEFINED,
        "use_estimate_learning_rate_once": UNDEFINED,
        "use_histogram_matching": True,
        "interpolation_parameters": UNDEFINED,
        "transform_parameters": UNDEFINED,
        "restrict_deformation": UNDEFINED,
        "number_of_iterations": UNDEFINED,
        "smoothing_sigmas": UNDEFINED,
        "sigma_units": UNDEFINED,
        "shrink_factors": UNDEFINED,
        "convergence_threshold": [1e-6],
        "convergence_window_size": [10],
        "output_warped_image": UNDEFINED,
        "output_inverse_warped_image": UNDEFINED,
        "num_threads": 1,
    }
    _linear = ("Rigid", "Affine", "Translation", "CompositeAffine", "Similarity")

    def __init__(self, inputs: Mapping[str, Any], cwd: str | os.PathLike[str]) -> None:
        super().__init__(inputs, cwd)
        self._quantiles_done = False
        self._coerce()
        for name in ("fixed_image_mask", "moving_image_mask"):
            if self.defined(name):
                raise ValueError(
                    f"Registration: {name} is an input of ANTs <= 2.1.0 (larmorx's "
                    f"antsRegistration is 2.6.5): use {name}s"
                )

    def reset(self) -> None:
        self._quantiles_done = False

    def _coerce(self) -> None:
        v = self.inputs
        for name in ("fixed_image", "moving_image", "initial_moving_transform"):
            if isdefined(v.get(name, UNDEFINED)):
                v[name] = [os.fspath(p) for p in ensure_list(v[name]) or []]
        if isdefined(v["transform_parameters"]):
            v["transform_parameters"] = [
                _transform_parameters(p) for p in v["transform_parameters"]
            ]
        for name in ("smoothing_sigmas", "restrict_deformation"):
            if isdefined(v[name]):
                v[name] = _floats2(v[name])
        for name in ("metric_weight", "sampling_percentage"):
            if isdefined(v[name]):
                v[name] = _floats_or_lists(v[name])
        if isdefined(v["convergence_threshold"]):
            v["convergence_threshold"] = [_float(x) for x in v["convergence_threshold"]]
        for name in ("winsorize_lower_quantile", "winsorize_upper_quantile"):
            v[name] = _float(v[name])
        if isdefined(v["interpolation_parameters"]):
            v["interpolation_parameters"] = _interpolation_parameters(v["interpolation_parameters"])

    # Registration._format_metric
    def _format_metric(self, index: int) -> list[str]:
        name_input = self.get("metric")[index]
        stage: dict[str, Any] = {
            "fixed_image": self.get("fixed_image")[0],
            "moving_image": self.get("moving_image")[0],
            "metric": name_input,
            "weight": self.get("metric_weight")[index],
            "radius_or_bins": self.get("radius_or_number_of_bins")[index],
            "optional": self.get("radius_or_number_of_bins")[index],
        }
        strategy = self.get("sampling_strategy")
        if isdefined(strategy) and strategy:
            s = strategy[index]
            if s:
                stage["sampling_strategy"] = s
        percentage = self.get("sampling_percentage")
        if isdefined(percentage) and percentage:
            p = percentage[index]
            if p:
                stage["sampling_percentage"] = p
        if isinstance(name_input, list):
            specs = []
            fixed, moving = self.get("fixed_image"), self.get("moving_image")
            for i in range(len(name_input)):
                temp = {k: v[i] for k, v in stage.items()}
                temp["fixed_image"] = fixed[0] if len(fixed) == 1 else fixed[i]
                temp["moving_image"] = moving[0] if len(moving) == 1 else moving[i]
                specs.append(temp)
        else:
            specs = [stage]
        return [self._format_metric_argument(**spec) for spec in specs]

    @staticmethod
    def _format_metric_argument(**kw: Any) -> str:
        retval = "%s[ %s, %s, %g, %d" % (
            kw["metric"],
            kw["fixed_image"],
            kw["moving_image"],
            kw["weight"],
            kw["radius_or_bins"],
        )
        if "sampling_strategy" in kw:
            strategy = kw["sampling_strategy"]
        elif "sampling_percentage" in kw:
            strategy = "None"  # Registration.DEF_SAMPLING_STRATEGY
        else:
            strategy = None
        if strategy:
            retval += f", {strategy}"
            if "sampling_percentage" in kw:
                retval += ", %g" % kw["sampling_percentage"]
        return retval + " ]"

    def _format_transform(self, index: int) -> str:
        parameters = ", ".join(str(e) for e in self.get("transform_parameters")[index])
        return f"{self.get('transforms')[index]}[ {parameters} ]"

    def _format_convergence(self, ii: int) -> str:
        iters = _xarray(self.get("number_of_iterations")[ii])
        thr, win = self.get("convergence_threshold"), self.get("convergence_window_size")
        value = thr[ii] if len(thr) > ii else thr[0]
        ws = win[ii] if len(win) > ii else win[0]
        return "[ %s, %g, %d ]" % (iters, value, ws)

    def _format_registration(self) -> str:
        out: list[str] = []
        for ii in range(len(self.get("transforms"))):
            out.append(f"--transform {self._format_transform(ii)}")
            out.extend(f"--metric {m}" for m in self._format_metric(ii))
            out.append(f"--convergence {self._format_convergence(ii)}")
            sigmas = _xarray(self.get("smoothing_sigmas")[ii])
            if self.defined("sigma_units"):
                out.append(f"--smoothing-sigmas {sigmas}{self.get('sigma_units')[ii]}")
            else:
                out.append(f"--smoothing-sigmas {sigmas}")
            out.append(f"--shrink-factors {_xarray(self.get('shrink_factors')[ii])}")
            hist = self.get("use_histogram_matching")
            if isdefined(hist):
                histval = hist if isinstance(hist, bool) else hist[ii]
                out.append("--use-histogram-matching %d" % histval)
            if self.defined("restrict_deformation"):
                rd = _xarray(self.get("restrict_deformation")[ii])
                out.append(f"--restrict-deformation {rd}")
            fixed_m, moving_m = self.get("fixed_image_masks"), self.get("moving_image_masks")
            if isdefined(fixed_m) or isdefined(moving_m):
                if isdefined(fixed_m):
                    fm = ensure_list(fixed_m) or []
                    fixed_mask = fm[ii if len(fm) > 1 else 0]
                else:
                    fixed_mask = "NULL"
                if isdefined(moving_m):
                    mm = ensure_list(moving_m) or []
                    moving_mask = mm[ii if len(mm) > 1 else 0]
                else:
                    moving_mask = "NULL"
                out.append(f"--masks [ {fixed_mask}, {moving_mask} ]")
        return " ".join(out)

    def _outputfilename(self, inverse: bool = False) -> str | None:
        prefix = self.get("output_transform_prefix")
        if not inverse:
            value = self.get("output_warped_image")
            if isdefined(value) and value:
                return f"{prefix}_Warped.nii.gz" if isinstance(value, bool) else os.fspath(value)
            return None
        value = self.get("output_inverse_warped_image")
        if isdefined(value) and value:
            return f"{prefix}_InverseWarped.nii.gz" if isinstance(value, bool) else os.fspath(value)
        return None

    def _winsorize(self) -> str:
        lo, hi = self.get("winsorize_lower_quantile"), self.get("winsorize_upper_quantile")
        if not hi > lo:
            raise RuntimeError("Upper bound MUST be more than lower bound: %g > %g" % (hi, lo))
        self._quantiles_done = True
        return f"--winsorize-image-intensities [ {lo}, {hi} ]"

    def format_arg(self, t: _Trait, value: Any) -> str | None:
        name = t.name
        if name == "transforms":
            return self._format_registration()
        if name == "initial_moving_transform":
            n = len(value)
            flags = [0] * n
            inv = self.get("invert_initial_moving_transform")
            if isdefined(inv):
                if len(inv) != n:
                    raise ValueError(
                        'Inputs "initial_moving_transform" and "invert_initial_moving_transform"'
                        "should have the same length."
                    )
                flags = list(inv)
            items = ["[ %s, %d ]" % (x, int(f)) for x, f in zip(value, flags, strict=False)]
            return " ".join(["--initial-moving-transform", *items])
        if name == "initial_moving_transform_com":
            return "--initial-moving-transform [ %s, %s, %d ]" % (
                self.get("fixed_image")[0],
                self.get("moving_image")[0],
                value,
            )
        if name == "interpolation":
            params = self.get("interpolation_parameters")
            if value in ("BSpline", "MultiLabel", "Gaussian", "GenericLabel") and isdefined(params):
                return "--interpolation {}[ {} ]".format(value, ", ".join(str(p) for p in params))
            return f"--interpolation {value}"
        if name == "output_transform_prefix":
            out, inv = self._outputfilename(False), self._outputfilename(True)
            value, out, inv = self.in_cwd(value), self.in_cwd(out), self.in_cwd(inv)
            if out and inv:
                return f"--output [ {value}, {out}, {inv} ]"
            if out:
                return f"--output [ {value}, {out} ]"
            return f"--output {value}"
        if name in ("winsorize_upper_quantile", "winsorize_lower_quantile"):
            if not self._quantiles_done:
                return self._winsorize()
            self._quantiles_done = False
            return ""
        return super().format_arg(t, value)

    def _filenames(self, prefix: str, count: int, transform: str, inverse: bool = False):
        low = {
            "Rigid": "Rigid.mat",
            "Affine": "Affine.mat",
            "GenericAffine": "GenericAffine.mat",
            "CompositeAffine": "Affine.mat",
            "Similarity": "Similarity.mat",
            "Translation": "Translation.mat",
            "BSpline": "BSpline.txt",
            "Initial": "DerivedInitialMovingTranslation.mat",
        }
        if transform in low:
            suffix, mode = low[transform], inverse
        else:
            suffix, mode = ("InverseWarp.nii.gz" if inverse else "Warp.nii.gz"), False
        return "%s%d%s" % (prefix, count, suffix), mode

    def list_outputs(self) -> dict[str, Any]:
        out: dict[str, Any] = {
            "forward_transforms": [],
            "forward_invert_flags": [],
            "reverse_transforms": [],
            "reverse_invert_flags": [],
        }
        prefix = self.get("output_transform_prefix")
        init = self.get("initial_moving_transform")
        inv_init = [False] * len(init)
        if self.defined("invert_initial_moving_transform"):
            inv_init = list(self.get("invert_initial_moving_transform"))
        transforms = self.get("transforms")
        if self.get("write_composite_transform"):
            out["composite_transform"] = self.abspath(prefix + "Composite.h5")
            out["inverse_composite_transform"] = self.abspath(prefix + "InverseComposite.h5")
        elif not self.get("collapse_output_transforms"):
            count = 0
            if isdefined(init):
                out["forward_transforms"] += list(init)
                out["forward_invert_flags"] += inv_init
                out["reverse_transforms"] = list(init) + out["reverse_transforms"]
                out["reverse_invert_flags"] = [not e for e in inv_init] + out[
                    "reverse_invert_flags"
                ]
                count += len(init)
            elif self.defined("initial_moving_transform_com"):
                f, _ = self._filenames(prefix, count, "Initial")
                r, _ = self._filenames(prefix, count, "Initial", True)
                out["forward_transforms"].append(self.abspath(f))
                out["forward_invert_flags"].append(False)
                out["reverse_transforms"].insert(0, self.abspath(r))
                out["reverse_invert_flags"].insert(0, True)
                count += 1
            for t in transforms:
                f, fm = self._filenames(prefix, count, t)
                r, rm = self._filenames(prefix, count, t, True)
                out["forward_transforms"].append(self.abspath(f))
                out["forward_invert_flags"].append(fm)
                out["reverse_transforms"].insert(0, self.abspath(r))
                out["reverse_invert_flags"].insert(0, rm)
                count += 1
        else:
            is_linear = [t in self._linear for t in transforms]
            if isdefined(init) or self.defined("initial_moving_transform_com"):
                is_linear.insert(0, True)
            collapse = []
            if any(is_linear):
                collapse.append("GenericAffine")
            if not all(is_linear):
                collapse.append("SyN")
            for count, t in enumerate(collapse):
                f, fm = self._filenames(prefix, count, t, inverse=False)
                r, rm = self._filenames(prefix, count, t, inverse=True)
                out["forward_transforms"].append(self.abspath(f))
                out["forward_invert_flags"].append(fm)
                out["reverse_transforms"].append(self.abspath(r))
                out["reverse_invert_flags"].append(rm)
        warped, inverse = self._outputfilename(False), self._outputfilename(True)
        if warped:
            out["warped_image"] = self.abspath(warped)
        if inverse:
            out["inverse_warped_image"] = self.abspath(inverse)
        if len(self.get("save_state")):
            out["save_state"] = self.abspath(self.get("save_state"))
        out["reverse_forward_transforms"] = out["forward_transforms"][::-1]
        out["reverse_forward_invert_flags"] = out["forward_invert_flags"][::-1]
        return out


def registration(
    *,
    cwd: str | os.PathLike[str],
    from_file: str | os.PathLike[str] | Mapping[str, Any] | None = None,
    **inputs: Any,
) -> Command:
    """nipype's ``Registration(from_file=..., **inputs)`` (niworkflows'
    ``FixHeaderRegistration``): ``antsRegistration``'s command line with the settings file's
    values and ``inputs`` over them, as nipype sets them: the interface's own inputs again after
    ``load_inputs_from_json`` (``BaseInterface.__init__``), and the inputs connected or set later
    (``fixed_image``, ``moving_image``, masks, sdcflows' SyN ``transform_parameters`` and
    ``metric_weight``, ...) over both. Outputs: ``forward_transforms``, ``reverse_transforms``
    and their invert flags, ``composite_transform``, ``warped_image``,
    ``inverse_warped_image``."""
    values: dict[str, Any] = {}
    if from_file is not None:
        settings = dict(from_file) if isinstance(from_file, Mapping) else load_settings(from_file)
        values.update(settings)
    values.update(inputs)
    return _Registration(values, cwd).command()


class _ApplyTransforms(_AntsCommand):
    """``ants.ApplyTransforms`` (niworkflows' ``FixHeaderApplyTransforms``)."""

    program = "antsApplyTransforms"
    traits = (
        _Trait("args", "%s"),
        _Trait("default_value", "--default-value %g", default=0.0),
        _Trait("dimension", "--dimensionality %d"),
        _Trait("float", "--float %d", default=False),
        _Trait("input_image", "--input %s", "path"),
        _Trait("input_image_type", "--input-image-type %d"),
        _Trait("interpolation", "%s", default="Linear"),
        _Trait("output_image", "--output %s", genfile=True),
        _Trait("reference_image", "--reference-image %s", "path"),
        _Trait("transforms", "%s"),
    )
    defaults: ClassVar[dict[str, Any]] = {
        "out_postfix": "_trans",
        "interpolation_parameters": UNDEFINED,
        "invert_transform_flags": UNDEFINED,
        "print_out_composite_warp_file": UNDEFINED,
        "num_threads": 1,
    }

    def __init__(self, inputs: Mapping[str, Any], cwd: str | os.PathLike[str]) -> None:
        super().__init__(inputs, cwd)
        t = self.get("transforms")
        if isdefined(t):
            self.inputs["transforms"] = [os.fspath(x) for x in ensure_list(t) or []]
        if self.defined("interpolation_parameters"):
            self.inputs["interpolation_parameters"] = _interpolation_parameters(
                self.get("interpolation_parameters")
            )
        if self.defined("default_value"):
            self.inputs["default_value"] = _float(self.get("default_value"))

    def gen_filename(self, name: str) -> Any:
        if name == "output_image":
            output = self.get("output_image")
            if not isdefined(output):
                _, stem, ext = split_filename(self.get("input_image"))
                output = stem + self.get("out_postfix") + ext
            return output
        return None

    def format_arg(self, t: _Trait, value: Any) -> str | None:
        if t.name == "output_image":
            composite = self.get("print_out_composite_warp_file")
            output = self.in_cwd(self.gen_filename("output_image"))
            if isdefined(composite):
                return "--output [ %s, %d ]" % (output, int(composite))
            return f"--output {output}"
        if t.name == "transforms":
            flags = self.get("invert_transform_flags")
            if not isdefined(flags):
                flags = [False] * len(value)
            elif len(value) != len(flags):
                raise ValueError(
                    "ERROR: The invert_transform_flags list must have the same number "
                    "of entries as the transforms list."
                )
            return " ".join(
                f"--transform [ {x}, 1 ]" if f else f"--transform {x}"
                for x, f in zip(value, flags, strict=True)
            )
        if t.name == "interpolation":
            params = self.get("interpolation_parameters")
            if value in ("BSpline", "MultiLabel", "Gaussian", "GenericLabel") and isdefined(params):
                return "--interpolation {}[ {} ]".format(value, ", ".join(str(p) for p in params))
            return f"--interpolation {value}"
        return super().format_arg(t, value)

    def list_outputs(self) -> dict[str, Any]:
        return {"output_image": self.abspath(self.gen_filename("output_image"))}


def apply_transforms(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``ApplyTransforms`` (niworkflows' ``FixHeaderApplyTransforms``, which also takes
    ``"identity"`` among the transforms; after the run it copies the reference's x-forms onto
    the output: :func:`larmorx.niworkflows.copy_xform`). Output: ``output_image``."""
    return _ApplyTransforms(inputs, cwd).command()


class _N4(_AntsCommand):
    """``ants.N4BiasFieldCorrection``."""

    program = "N4BiasFieldCorrection"
    traits = (
        _Trait("args", "%s"),
        _Trait("bspline_fitting_distance", "--bspline-fitting %s"),
        _Trait("dimension", "-d %d", default=3),
        _Trait("histogram_sharpening", "--histogram-sharpening [%g,%g,%d]"),
        _Trait("input_image", "--input-image %s", "path"),
        _Trait("mask_image", "--mask-image %s", "path"),
        _Trait("n_iterations", "--convergence %s", "list"),
        _Trait(
            "output_image",
            "--output %s",
            name_source="input_image",
            name_template="%s_corrected",
            keep_extension=True,
        ),
        _Trait("rescale_intensities", "-r", "bool", default=False),
        _Trait("shrink_factor", "--shrink-factor %d"),
        _Trait("weight_image", "--weight-image %s", "path"),
    )
    defaults: ClassVar[dict[str, Any]] = {
        "bspline_order": UNDEFINED,
        "convergence_threshold": UNDEFINED,
        "save_bias": False,
        "bias_image": UNDEFINED,
        "copy_header": False,
        "num_threads": 1,
    }

    def __init__(self, inputs: Mapping[str, Any], cwd: str | os.PathLike[str]) -> None:
        super().__init__(inputs, cwd)
        self._bias: str | None = None
        if self.get("save_bias") or self.defined("bias_image"):
            bias = self.get("bias_image")
            if not isdefined(bias):
                bias = fname_presuffix(os.path.basename(self.get("input_image")), suffix="_bias")
            self._bias = bias
        for name in ("bspline_fitting_distance", "convergence_threshold"):
            if self.defined(name):
                self.inputs[name] = _float(self.get(name))

    def parse_inputs(self, skip: Sequence[str] = ()) -> list[str]:
        return super().parse_inputs([*skip, "save_bias", "bias_image"])

    def format_arg(self, t: _Trait, value: Any) -> str | None:
        if t.name == "output_image" and self._bias:
            return t.argstr % f"[ {value}, {self.in_cwd(self._bias)} ]"
        if t.name == "bspline_fitting_distance":
            if self.defined("bspline_order"):
                return t.argstr % ("[ %g, %d ]" % (value, self.get("bspline_order")))
            return t.argstr % ("[ %g ]" % value)
        if t.name == "n_iterations":
            its = _xarray([str(e) for e in value])
            if self.defined("convergence_threshold"):
                return t.argstr % "[ {}, {:g} ]".format(its, self.get("convergence_threshold"))
            return t.argstr % f"[ {its} ]"
        return super().format_arg(t, value)

    def list_outputs(self) -> dict[str, Any]:
        out = super().list_outputs()
        if self._bias:
            out["bias_image"] = self.abspath(self._bias)
        return out


def n4_bias_field_correction(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``N4BiasFieldCorrection``. Outputs: ``output_image`` (``bias_image``)."""
    return _N4(inputs, cwd).command()


class _ImageMath(_AntsCommand):
    """``ants.ImageMath``."""

    program = "ImageMath"
    traits = (
        _Trait("args", "%s", position=-1),
        _Trait("dimension", "%d", position=1, default=3),
        _Trait("op1", "%s", "path", position=-3),
        _Trait("op2", "%s", "compound", position=-2),
        _Trait("operation", "%s", position=3),
        _Trait(
            "output_image",
            "%s",
            "path",
            position=2,
            name_source="op1",
            name_template="%s_maths",
            keep_extension=True,
        ),
    )
    defaults: ClassVar[dict[str, Any]] = {"copy_header": True, "num_threads": 1}
    #: ``ImageMath._no_copy_header_operation``: operations whose header nipype never copies.
    no_copy_header = (
        "PadImage",
        "LabelStats",
        "SetTimeSpacing",
        "SetTimeSpacingWarp",
        "TriPlanarView",
    )

    def __init__(self, inputs: Mapping[str, Any], cwd: str | os.PathLike[str]) -> None:
        super().__init__(inputs, cwd)
        if self.get("operation") in self.no_copy_header:
            self.inputs["copy_header"] = False


def image_math(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``ImageMath`` (``copy_header`` True by default, except for the operations of
    ``no_copy_header``). Output: ``output_image``."""
    return _ImageMath(inputs, cwd).command()


class _DenoiseImage(_AntsCommand):
    """``ants.DenoiseImage``."""

    program = "DenoiseImage"
    traits = (
        _Trait("args", "%s"),
        _Trait("dimension", "-d %d"),
        _Trait("input_image", "-i %s", "path"),
        _Trait("noise_model", "-n %s", default="Gaussian"),
        _Trait(
            "output_image",
            "-o %s",
            "path",
            name_source="input_image",
            name_template="%s_noise_corrected",
            keep_extension=True,
        ),
        _Trait("shrink_factor", "-s %s", default=1),
        _Trait("verbose", "-v", "bool"),
    )
    defaults: ClassVar[dict[str, Any]] = {
        "save_noise": False,
        "noise_image": UNDEFINED,
        "copy_header": False,
        "num_threads": 1,
    }

    def _noise_trait(self) -> _Trait:
        return _Trait(
            "noise_image",
            "",
            name_source="input_image",
            name_template="%s_noise",
            keep_extension=True,
        )

    def filename_from_source(self, name: str, chain: list[str] | None = None) -> Any:
        if name == "noise_image":
            saved = self._trait
            self._trait = {**saved, "noise_image": self._noise_trait()}
            try:
                return super().filename_from_source(name, chain)
            finally:
                self._trait = saved
        return super().filename_from_source(name, chain)

    def format_arg(self, t: _Trait, value: Any) -> str | None:
        if t.name == "output_image" and (self.get("save_noise") or self.defined("noise_image")):
            newval = "[ {}, {} ]".format(
                self.in_cwd(self.filename_from_source("output_image")),
                self.in_cwd(self.filename_from_source("noise_image")),
            )
            return t.argstr % newval
        return super().format_arg(t, value)

    def list_outputs(self) -> dict[str, Any]:
        # nipype's CommandLine._list_outputs lists every name_source input's file name, so
        # noise_image is listed whether or not the program writes it (save_noise False and no
        # noise_image: no such file; PIPE-38, rule C: nipype's listing kept)
        out = super().list_outputs()
        out["noise_image"] = self.abspath(self.filename_from_source("noise_image"))
        return out


def denoise_image(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``DenoiseImage`` (sdcflows' version copies the input's header onto the output:
    ``copy_header``). Outputs: ``output_image`` (``noise_image``)."""
    return _DenoiseImage(inputs, cwd).command()


# ------------------------------------------------------------------------------------------
# FSL (nipype/interfaces/fsl)

_FSL_EXT = {
    "NIFTI": ".nii",
    "NIFTI_PAIR": ".img",
    "NIFTI_GZ": ".nii.gz",
    "NIFTI_PAIR_GZ": ".img.gz",
}


class _FslCommand(_Interface):
    def output_type(self) -> str:
        t = self.get("output_type")
        return "NIFTI_GZ" if not isdefined(t) else t

    def environ(self) -> dict[str, str]:
        return {"FSLOUTPUTTYPE": self.output_type()}

    def ext(self) -> str:
        return _FSL_EXT[self.output_type()]

    def overload_extension(self, value: str, name: str) -> str:
        return value + self.ext()

    def gen_fname(
        self,
        basename: str,
        cwd: str | None = None,
        suffix: str | None = None,
        change_ext: bool = True,
        ext: str | None = None,
    ) -> str:
        """``FSLCommand._gen_fname``."""
        if basename == "":
            raise ValueError(
                f"Unable to generate filename for command {self.program}. basename is not set!"
            )
        if cwd is None:
            cwd = self.cwd
        if ext is None:
            ext = self.ext()
        if change_ext:
            suffix = f"{suffix}{ext}" if suffix else ext
        if suffix is None:
            suffix = ""
        return fname_presuffix(basename, suffix=suffix, use_ext=False, newpath=cwd)


_BET_EXCLUSIVE = (
    "functional",
    "reduce_bias",
    "robust",
    "padding",
    "remove_eyes",
    "surfaces",
    "t2_guided",
)


class _Bet(_FslCommand):
    """``fsl.BET``."""

    program = "bet"
    traits = (
        _Trait("args", "%s"),
        _Trait("center", "-c %s", "list"),
        _Trait("frac", "-f %.2f"),
        _Trait("functional", "-F", "bool", xor=_BET_EXCLUSIVE),
        _Trait("in_file", "%s", "path", position=0),
        _Trait("mask", "-m", "bool"),
        _Trait("mesh", "-e", "bool"),
        _Trait("no_output", "-n", "bool"),
        _Trait("out_file", "%s", "path", position=1, genfile=True),
        _Trait("outline", "-o", "bool"),
        _Trait("padding", "-Z", "bool", xor=_BET_EXCLUSIVE),
        _Trait("radius", "-r %d"),
        _Trait("reduce_bias", "-B", "bool", xor=_BET_EXCLUSIVE),
        _Trait("remove_eyes", "-S", "bool", xor=_BET_EXCLUSIVE),
        _Trait("robust", "-R", "bool", xor=_BET_EXCLUSIVE),
        _Trait("skull", "-s", "bool"),
        _Trait("surfaces", "-A", "bool", xor=_BET_EXCLUSIVE),
        _Trait("t2_guided", "-A2 %s", "path", xor=_BET_EXCLUSIVE),
        _Trait("threshold", "-t", "bool"),
        _Trait("vertical_gradient", "-g %.2f"),
    )
    defaults: ClassVar[dict[str, Any]] = {"output_type": "NIFTI_GZ"}

    def __init__(self, inputs: Mapping[str, Any], cwd: str | os.PathLike[str]) -> None:
        super().__init__(inputs, cwd)
        chosen = [n for n in _BET_EXCLUSIVE if self.defined(n) and self.get(n) is not False]
        if len(chosen) > 1:
            raise ValueError(f"BET: the inputs {chosen} are mutually exclusive")

    def _gen_outfilename(self) -> str:
        out = self.get("out_file")
        if not isdefined(out) and self.defined("in_file"):
            out = self.gen_fname(self.get("in_file"), suffix="_brain")
            return _bet_relpath(out, self.cwd)
        return out

    def gen_filename(self, name: str) -> Any:
        return self._gen_outfilename() if name == "out_file" else None

    def format_arg(self, t: _Trait, value: Any) -> str | None:
        formatted = super().format_arg(t, value)
        if t.name == "in_file" and formatted is not None:
            return _bet_relpath(formatted, self.cwd)
        return formatted

    def list_outputs(self) -> dict[str, Any]:
        out: dict[str, Any] = {"out_file": self.abspath(self._gen_outfilename())}
        basename, cwd = os.path.basename(out["out_file"]), os.path.dirname(out["out_file"])

        def g(suffix: str, change_ext: bool = True) -> str:
            return self.gen_fname(basename, cwd=cwd, suffix=suffix, change_ext=change_ext)

        on = lambda n: self.defined(n) and bool(self.get(n))  # noqa: E731
        if on("mesh") or on("surfaces"):
            out["meshfile"] = g("_mesh.vtk", change_ext=False)
        if on("mask") or on("reduce_bias"):
            out["mask_file"] = g("_mask")
        if on("outline"):
            out["outline_file"] = g("_overlay")
        if on("surfaces"):
            for k in ("inskull_mask", "inskull_mesh", "outskull_mask", "outskull_mesh"):
                out[f"{k}_file"] = g(f"_{k}")
            out["outskin_mask_file"] = g("_outskin_mask")
            out["outskin_mesh_file"] = g("_outskin_mesh")
            out["skull_mask_file"] = g("_skull_mask")
        if on("skull"):
            out["skull_file"] = g("_skull")
        if on("no_output"):
            out["out_file"] = UNDEFINED
        return out


def bet(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``BET`` (the input and output names relative to the node directory, as nipype
    writes them against BET's limit on long paths). Outputs: ``out_file``, ``mask_file``, ..."""
    return _Bet(inputs, cwd).command()


class _MultiImageMaths(_FslCommand):
    """``fsl.MultiImageMaths``."""

    program = "fslmaths"
    traits = (
        _Trait("args", "%s"),
        _Trait("in_file", "%s", "path", position=2),
        _Trait("internal_datatype", "-dt %s", position=1),
        _Trait("nan2zeros", "-nan", "bool", position=3),
        _Trait("op_string", "%s", position=4),
        _Trait("out_file", "%s", "path", position=-2, genfile=True),
        _Trait("output_datatype", "-odt %s", position=-1),
    )
    defaults: ClassVar[dict[str, Any]] = {"output_type": "NIFTI_GZ", "operand_files": UNDEFINED}
    suffix = "_maths"

    def format_arg(self, t: _Trait, value: Any) -> str | None:
        if t.name == "op_string":
            return value % tuple(os.fspath(p) for p in self.get("operand_files"))
        return super().format_arg(t, value)

    def list_outputs(self) -> dict[str, Any]:
        out = self.get("out_file")
        if not isdefined(out):
            out = self.gen_fname(self.get("in_file"), suffix=self.suffix)
        return {"out_file": self.abspath(out)}

    def gen_filename(self, name: str) -> Any:
        return self.list_outputs()["out_file"] if name == "out_file" else None


def multi_image_maths(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``MultiImageMaths`` (``op_string`` with ``%s`` for each of ``operand_files``).
    Output: ``out_file``."""
    return _MultiImageMaths(inputs, cwd).command()


class _BinaryMaths(_MultiImageMaths):
    """``fsl.BinaryMaths``."""

    traits = (
        _Trait("args", "%s"),
        _Trait("in_file", "%s", "path", position=2),
        _Trait("internal_datatype", "-dt %s", position=1),
        _Trait("nan2zeros", "-nan", "bool", position=3),
        _Trait("operand_file", "%s", "path", position=5, xor=("operand_value",)),
        _Trait("operand_value", "%.8f", position=5, xor=("operand_file",)),
        _Trait("operation", "-%s", position=4),
        _Trait("out_file", "%s", "path", position=-2, genfile=True),
        _Trait("output_datatype", "-odt %s", position=-1),
    )
    defaults: ClassVar[dict[str, Any]] = {"output_type": "NIFTI_GZ"}

    def format_arg(self, t: _Trait, value: Any) -> str | None:
        return _FslCommand.format_arg(self, t, value)


def binary_maths(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``BinaryMaths`` (``fslmaths <in> -<operation> <operand> <out>``; ``operation``:
    ``add``, ``sub``, ``mul``, ``div``, ``rem``, ``max``, ``min``). Output: ``out_file``."""
    return _BinaryMaths(inputs, cwd).command()


class _Topup(_FslCommand):
    """``fsl.TOPUP``."""

    program = "topup"
    traits = (
        _Trait("args", "%s"),
        _Trait("config", "--config=%s", default="b02b0.cnf"),
        _Trait(
            "encoding_direction",
            "--datain=%s",
            "list",
            xor=("encoding_file",),
            requires=("readout_times",),
        ),
        _Trait("encoding_file", "--datain=%s", "path", xor=("encoding_direction",)),
        _Trait("estmov", "--estmov=%d"),
        _Trait("fwhm", "--fwhm=%f"),
        _Trait("in_file", "--imain=%s", "path"),
        _Trait("interp", "--interp=%s"),
        _Trait("max_iter", "--miter=%d"),
        _Trait("minmet", "--minmet=%d"),
        _Trait("numprec", "--numprec=%s"),
        _Trait("out_base", "--out=%s", "path", name_source="in_file", name_template="%s_base"),
        _Trait(
            "out_corrected",
            "--iout=%s",
            "path",
            name_source="in_file",
            name_template="%s_corrected",
        ),
        _Trait("out_field", "--fout=%s", "path", name_source="in_file", name_template="%s_field"),
        _Trait("out_jac_prefix", "--jacout=%s", default="jac"),
        _Trait(
            "out_logfile",
            "--logout=%s",
            "path",
            name_source="in_file",
            name_template="%s_topup.log",
            keep_extension=True,
        ),
        _Trait("out_mat_prefix", "--rbmout=%s", default="xfm"),
        _Trait("out_warp_prefix", "--dfout=%s", default="warpfield"),
        _Trait("reg_lambda", "--lambda=%0.f"),
        _Trait("regmod", "--regmod=%s"),
        _Trait("regrid", "--regrid=%d"),
        _Trait("scale", "--scale=%d"),
        _Trait("splineorder", "--splineorder=%d"),
        _Trait("ssqlambda", "--ssqlambda=%d"),
        _Trait("subsamp", "--subsamp=%d"),
        _Trait("warp_res", "--warpres=%f"),
    )
    defaults: ClassVar[dict[str, Any]] = {"output_type": "NIFTI_GZ", "readout_times": UNDEFINED}

    def _encfilename(self) -> str:
        return os.path.join(self.cwd, "%s_encfile.txt" % split_filename(self.get("in_file"))[1])

    def _encfile_text(self) -> str:
        directions = list(self.get("encoding_direction"))
        durations = list(self.get("readout_times"))
        if len(directions) != len(durations):
            if len(durations) != 1:
                raise ValueError(
                    "Readout time must be a float or match thelength of encoding directions"
                )
            durations = durations * len(directions)
        lines = []
        for idx, encdir in enumerate(directions):
            sign = -1.0 if encdir.endswith("-") else 1.0
            row = [float(v == encdir[0]) * sign for v in "xyz"] + [float(durations[idx])]
            lines.append("%d %d %d %.8f" % tuple(row))
        return "".join(line + "\n" for line in lines)

    def overload_extension(self, value: str, name: str) -> str:
        return value if name == "out_base" else super().overload_extension(value, name)

    def format_arg(self, t: _Trait, value: Any) -> str | None:
        if t.name == "encoding_direction":
            return t.argstr % self._encfilename()
        if t.name == "out_base":
            path, _, _ = split_filename(value)
            if path != "" and not os.path.exists(path):
                raise ValueError("out_base path must exist if provided")
        return super().format_arg(t, value)

    def files(self) -> dict[str, str]:
        if self.defined("encoding_direction"):
            return {self._encfilename(): self._encfile_text()}
        return {}

    def list_outputs(self) -> dict[str, Any]:
        out = super().list_outputs()
        del out["out_base"]
        base_path: str | None = None
        if self.defined("out_base"):
            base_path, base, _ = split_filename(self.get("out_base"))
            if base_path == "":
                base_path = None
        else:
            base = split_filename(self.get("in_file"))[1] + "_base"
        out["out_fieldcoef"] = self.gen_fname(base, suffix="_fieldcoef", cwd=base_path)
        out["out_movpar"] = self.gen_fname(base, suffix="_movpar", ext=".txt", cwd=base_path)
        from larmorx.io.nibabel_header import load_nibabel

        n_vols = int(load_nibabel(self.get("in_file"), scaled=False).data.shape[-1])
        ext = self.ext()

        def fmt(prefix: str, i: int, e: str) -> str:
            return self.abspath(f"{prefix}_{i:02d}{e}")

        rng = range(1, n_vols + 1)
        out["out_warps"] = [fmt(self.get("out_warp_prefix"), i, ext) for i in rng]
        out["out_jacs"] = [fmt(self.get("out_jac_prefix"), i, ext) for i in rng]
        out["out_mats"] = [fmt(self.get("out_mat_prefix"), i, ".mat") for i in rng]
        if self.defined("encoding_direction"):
            out["out_enc_file"] = self._encfilename()
        return out


def topup(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``TOPUP`` (its encoding file in :attr:`Command.files`, written by :func:`run`
    before the program). Outputs: ``out_fieldcoef``, ``out_movpar``, ``out_field``,
    ``out_corrected``, ``out_warps``, ``out_jacs``, ``out_mats``, ``out_enc_file``, ..."""
    return _Topup(inputs, cwd).command()


# ------------------------------------------------------------------------------------------
# AFNI (nipype/interfaces/afni)

_AFNI_EXT = {"NIFTI_GZ": ".nii.gz", "NIFTI": ".nii", "AFNI": ""}


class _AfniCommand(_Interface):
    def outputtype(self) -> str:
        t = self.get("outputtype")
        return "AFNI" if not isdefined(t) else t

    def overload_extension(self, value: str, name: str) -> str:
        path, base, _ = split_filename(value)
        return os.path.join(path, base + _AFNI_EXT[self.outputtype()])

    def list_outputs(self) -> dict[str, Any]:
        out = super().list_outputs()
        for name in [t.name for t in self.traits if t.name_source]:
            if out.get(name):
                _, _, ext = split_filename(out[name])
                if ext == "":
                    out[name] = out[name] + "+orig.BRIK"
        return out

    def environ(self) -> dict[str, str]:
        n = self.get("num_threads")
        return {"OMP_NUM_THREADS": str(1 if not isdefined(n) else n)}


class _Automask(_AfniCommand):
    """``afni.Automask``."""

    program = "3dAutomask"
    traits = (
        _Trait("args", "%s"),
        _Trait(
            "brain_file",
            "-apply_prefix %s",
            "path",
            name_source="in_file",
            name_template="%s_masked",
        ),
        _Trait("clfrac", "-clfrac %s"),
        _Trait("dilate", "-dilate %s"),
        _Trait("erode", "-erode %s"),
        _Trait("in_file", "%s", "path", position=-1),
        _Trait("out_file", "-prefix %s", "path", name_source="in_file", name_template="%s_mask"),
    )
    defaults: ClassVar[dict[str, Any]] = {"outputtype": "AFNI", "num_threads": 1}


def automask(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``Automask`` (``outputtype``: ``"AFNI"`` by default, as nipype's; niworkflows
    asks ``"NIFTI_GZ"``). Outputs: ``out_file``, ``brain_file``."""
    return _Automask(inputs, cwd).command()


class _Volreg(_AfniCommand):
    """``afni.Volreg``."""

    program = "3dvolreg"
    traits = (
        _Trait("args", "%s"),
        _Trait("basefile", "-base %s", "path", position=-6),
        _Trait("copyorigin", "-twodup", "bool"),
        _Trait("in_file", "%s", "path", position=-1),
        _Trait("in_weight_volume", "-weight '%s[%d]'", "compound"),
        _Trait("interp", "-%s"),
        _Trait(
            "md1d_file",
            "-maxdisp1D %s",
            "path",
            position=-4,
            name_source="in_file",
            name_template="%s_md.1D",
            keep_extension=True,
        ),
        _Trait(
            "oned_file",
            "-1Dfile %s",
            "path",
            name_source="in_file",
            name_template="%s.1D",
            keep_extension=True,
        ),
        _Trait(
            "oned_matrix_save",
            "-1Dmatrix_save %s",
            "path",
            name_source="in_file",
            name_template="%s.aff12.1D",
            keep_extension=True,
        ),
        _Trait("out_file", "-prefix %s", "path", name_source="in_file", name_template="%s_volreg"),
        _Trait("timeshift", "-tshift 0", "bool"),
        _Trait("verbose", "-verbose", "bool"),
        _Trait("zpad", "-zpad %d", position=-5),
    )
    defaults: ClassVar[dict[str, Any]] = {"outputtype": "AFNI", "num_threads": 1}

    def format_arg(self, t: _Trait, value: Any) -> str | None:
        if t.name == "in_weight_volume" and not isinstance(value, tuple):
            value = (value, 0)
        return super().format_arg(t, value)


def volreg(*, cwd: str | os.PathLike[str], **inputs: Any) -> Command:
    """nipype's ``Volreg``. Outputs: ``out_file``, ``md1d_file``, ``oned_file``,
    ``oned_matrix_save``."""
    return _Volreg(inputs, cwd).command()


# ------------------------------------------------------------------------------------------
# running a command through larmorx


_ANTS_PROGRAMS = ("antsRegistration", "N4BiasFieldCorrection", "ImageMath", "DenoiseImage")


def run(
    cmd: Command,
    *,
    implementation: str = "auto",
    n_threads: int | None = None,
    upstream_defects: str = "fix",
    ants_random_seed: int | None = None,
    on_message: Callable[[str], None] | None = None,
) -> Command:
    """Run ``cmd`` in its node directory (made) through larmorx's port of its program, as
    nipype runs it: ANTs' programs in this process (``n_threads``: nipype's ``NSLOTS`` when
    ``None``; ``antsApplyTransforms`` on files), FSL's ``bet``, ``fslmaths`` and ``topup`` through their replica (``larmorx-nc``)
    or clean-room original (``implementation``), AFNI's through their built-in replica, with
    nipype's ``FSLOUTPUTTYPE``. The interface's files (TOPUP's encoding file) are written
    first. A failure raises :class:`RuntimeError` with the program's messages; its notices
    and warnings become Python warnings. Returns ``cmd``.

    ``ants_random_seed`` is what ANTs reads from ``ANTS_RANDOM_SEED`` (fMRIPrep sets it from its
    ``--random-seed``): ``antsRegistration`` runs with ``--random-seed <seed>`` unless its line
    has one (ANTs' option wins over the variable, as here). ``None``: larmorx's fixed default
    seed (ANTs seeds from the clock without either: a result that changes from run to run is
    never reproduced, D13)."""
    import warnings

    from larmorx import _core, _defects, _replica

    _defects.check(upstream_defects)  # type: ignore[arg-type]
    os.makedirs(cmd.cwd, exist_ok=True)
    for path, text in cmd.files.items():
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    threads = n_threads
    if threads is None:
        threads = int(cmd.environ.get("NSLOTS", cmd.environ.get("OMP_NUM_THREADS", "1")))
    threads = max(int(threads), 1)

    def warn_lines(text: str) -> None:
        for line in text.splitlines():
            notice = _defects.parse_line(line)
            if notice is not None:
                warnings.warn(notice, stacklevel=3)
            elif line.startswith(("Warning: ", "*+ WARNING: ")):
                warnings.warn(line.split(": ", 1)[1], stacklevel=3)
            if on_message is not None:
                on_message(line)

    program = cmd.program
    if program == "antsApplyTransforms":
        code, out, err = _core.ants_file_program(program, cmd.run_argv(), threads, upstream_defects)
        warn_lines(err)
        if code != 0:
            raise RuntimeError(f"{program} failed (exit {code}): {err.strip()[-2000:]}")
        return cmd
    if program in _ANTS_PROGRAMS:
        argv = cmd.run_argv()
        seeded = ants_random_seed is not None and program == "antsRegistration"
        if seeded and "--random-seed" not in argv:
            argv += ["--random-seed", str(int(ants_random_seed))]
        code, out, err, _ = _core.ants_program(program, argv, {}, [], threads, upstream_defects)
        warn_lines(err)
        if code != 0:
            raise RuntimeError(f"{program} failed (exit {code}): {err.strip()[-2000:]}")
        return cmd
    if program in ("bet", "fslmaths", "topup"):
        tool = program
        env = {"FSLOUTPUTTYPE": cmd.environ.get("FSLOUTPUTTYPE", "NIFTI_GZ"), "PATH": cmd.cwd}
        selected = _replica.select("fsl", tool, implementation)
        if selected is None:
            pairs = [
                *env.items(),
                (_defects.ENV, upstream_defects),
                ("OMP_NUM_THREADS", str(threads)),
            ]
            code, out, err = _core.fsl_run_original(tool, cmd.argv(), pairs, cmd.cwd)
            warn_lines(err)
            if code != 0:
                raise RuntimeError(
                    f"larmorx fsl {tool} (the clean-room original) failed with exit code "
                    f"{code}: {(err + out).strip()[-2000:]}"
                )
            return cmd
        process = _replica.run(
            selected,
            cmd.argv(),
            cwd=cmd.cwd,
            n_threads=threads,
            upstream_defects=upstream_defects,
            env=env,  # type: ignore[arg-type]
            exclude=lambda name: name.upper().startswith("FSL"),
        )
        warn_lines(process.stderr)
        return cmd
    if program in ("3dAutomask", "3dvolreg"):
        if _replica.implementation_name(implementation, warn=False) == "clean-room":
            raise NotImplementedError(
                f"{program}'s command line runs through its built-in replica here "
                "(implementation='auto' or 'replica'); its clean-room original has a typed API "
                f"(larmorx.afni)"
            )
        from larmorx.afni import _common

        selected = _replica.select("afni", program, "replica")
        assert selected is not None
        _common.run_replica(
            selected,
            cmd.argv(),
            cmd.cwd,
            n_threads=threads,
            upstream_defects=upstream_defects,
            settings={},
        )
        return cmd
    raise NotImplementedError(f"larmorx does not run nipype's {program} command line yet")
