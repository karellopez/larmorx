# SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
"""``larmorx nipype run``: nipype 1.12.0's ``nipypecli run`` without nipype, click or traits.

Ported from nipype 1.12.0 (Apache-2.0, the nipype developers): ``scripts/cli.py`` (``run``: the
click command, its ``--list`` and ``-h``/``--help`` flags, its unknown options passed through),
``scripts/instance.py`` (``list_interfaces``), ``scripts/utils.py`` (``add_args_options``, whose
``add_argument`` calls ``interfaces.json`` holds as nipype makes them, with the help texts of
``get_trait_desc``), ``utils/nipype_cmd.py`` (``run_instance``), ``interfaces/base/specs.py``
(the inputs' and outputs' printing, ``_xor_warn``, ``_deprecated_warn``),
``interfaces/base/core.py`` (``_check_mandatory_inputs``, ``_check_requires``, ``_check_xor``,
``aggregate_outputs``), ``interfaces/base/traits_extension.py`` (the paths' and ``MultiObject``'s
validation) and ``utils/logger.py`` (the log lines' format); click 8.5.0's parser
(``click/parser.py``: the options' processing as ``ignore_unknown_options`` and
``allow_interspersed_args`` give it) and its usage errors; traits 7.2.0's validation of the
trait types these interfaces use and its error messages (``trait_errors.py``,
``trait_types.py``, ``trait_handlers.py``, ``trait_list_object.py``: BSD-3-Clause, Enthought).
``interfaces.json`` is written by ``validation/investigations/confounds/nipype_cli_spec.py``
from nipype, niworkflows 1.15.4 and fMRIPrep 25.2.7 themselves.

Corrected by default (``upstream_defects="reproduce"`` gives nipype's behaviour):

- PIPE-81: ``add_args_options`` gives an input of a compound trait (``traits.Either``) the
  argparse type of the input before it in name order (a variable left from the loop's
  previous iteration: ``CompCor``'s ``num_components`` reads a text, so a number of components
  is refused; ``save_pre_filter`` and ``save_metadata`` read a float, so a file name or
  ``True`` is refused), and cannot build the parser at all when the first input is compound
  (``UnboundLocalError``: niworkflows' ``TSV2JSON``) or a list has compound items
  (``AttributeError``: ``DictMerge``, fMRIPrep's ``FMRISummary``). larmorx builds every parser
  and reads a compound input's text by its alternatives, the first that takes it.
- PIPE-82: the texts of inputs that hold no text reach the trait unconverted: a tuple
  (``aCompCorMasks``' mandatory ``bold_zooms``, ``RegridToZooms``' ``zooms``, the plots'
  ``figsize``), an optional dictionary, an enumeration's values that are not text
  (``CompCor``'s ``pre_filter`` ``False``) are refused whatever is given; a list of booleans
  reads every text but the empty one as true (``type=bool``). larmorx reads a tuple as a Python
  literal (``2,2,2`` or ``(2, 2, 2)``), a dictionary as JSON (as nipype reads a mandatory one),
  an enumeration's value by its text, a boolean from ``True``/``1`` or ``False``/``0``.

Kept (PIPE-83, documented): a boolean input whose default is true is a ``store_true`` flag, so it
cannot be turned off from the command line.
"""

from __future__ import annotations

import argparse
import ast
import json
import logging
import os
import sys
import warnings
from collections.abc import Callable, Iterator, Mapping, Sequence
from contextlib import contextmanager
from functools import cache
from pathlib import Path
from typing import Any

#: The modules whose interfaces ``larmorx nipype run`` serves.
MODULES = (
    "nipype.algorithms.confounds",
    "niworkflows.interfaces.confounds",
    "niworkflows.interfaces.images",
    "niworkflows.interfaces.morphology",
    "niworkflows.interfaces.nibabel",
    "niworkflows.interfaces.utility",
    "niworkflows.interfaces.bold",
    "niworkflows.interfaces.patches",
    "fmriprep.interfaces.confounds",
)

_DEFECTS_ENV = "LARMORX_UPSTREAM_DEFECTS"


@cache
def _data() -> dict[str, Any]:
    with open(Path(__file__).with_name("interfaces.json"), encoding="utf-8") as f:
        return json.load(f)


def list_interfaces(module: str) -> list[str]:
    """nipype's ``list_interfaces(module)``: the names of the interface classes in ``module``'s
    namespace, sorted (those it imports included), as ``--list`` prints them; ``module`` is one
    of :data:`MODULES` (a ``ValueError`` otherwise)."""
    if module not in MODULES:
        raise ValueError(_module_message(module))
    return list(_data()["modules"][module]["interfaces"])


def ported_interfaces() -> dict[str, list[str]]:
    """The interfaces ``larmorx nipype run`` runs, per module of :data:`MODULES` (the others of
    a module are listed and their help printed, but running one is an error)."""
    from larmorx.nipype._adapters import ADAPTERS

    return {m: [n for n in list_interfaces(m) if (m, n) in ADAPTERS] for m in MODULES}


def _module_message(module: str) -> str:
    return (
        f"larmorx does not run the interfaces of '{module}' (larmorx nipype run serves "
        + ", ".join(MODULES)
        + ")"
    )


# --- nipype's Undefined and the error lines of a traceback ---------------------------------------


class _UndefinedType:
    """nipype's ``Undefined``: printed ``<undefined>``, false."""

    _instance: _UndefinedType | None = None

    def __new__(cls) -> _UndefinedType:
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __repr__(self) -> str:
        return "<undefined>"

    __str__ = __repr__

    def __bool__(self) -> bool:
        return False


UNDEFINED = _UndefinedType()


def _defined(v: Any) -> bool:
    return v is not UNDEFINED


class _Raised(Exception):
    """An exception nipype's command line ends with (a traceback): its type's qualified name
    and its message."""

    def __init__(self, name: str, message: str) -> None:
        super().__init__(message)
        self.name = name
        self.message = message


#: larmorx's exception types and the original's they stand for (the traceback's last line).
_ORIGINAL_TYPES = {
    "larmorx.confounds._tables.EmptyDataError": "pandas.errors.EmptyDataError",
    "larmorx.confounds._tables.ParserError": "pandas.errors.ParserError",
    "larmorx.confounds.motion.TransformFileError": "nitransforms.io.base.TransformFileError",
}


def _type_name(t: type) -> str:
    mod = t.__module__
    name = t.__qualname__ if mod in ("builtins", "__main__") else f"{mod}.{t.__qualname__}"
    return _ORIGINAL_TYPES.get(name, name)


def _traceback(name: str, message: str) -> str:
    """A traceback's first and last lines, as larmorx's Python programs print nipype's."""
    return (
        "Traceback (most recent call last):\n" + (f"{name}: {message}" if message else name) + "\n"
    )


# --- traits' validation ------------------------------------------------------------------------


class _Invalid(Exception):
    """A value a trait refuses (``is_item``: an element of a list)."""

    def __init__(self, info: str, value: Any, is_item: bool = False) -> None:
        super().__init__(info)
        self.info, self.value, self.is_item = info, value, is_item


def _article(name: str) -> str:
    """traits' ``add_article``."""
    return ("an " if name[:1].lower() in "aeiou" else "a ") + name


def _trait_error(spec_class: str, name: str, err: _Invalid) -> _Raised:
    """traits' ``TraitError`` message for ``err`` on input ``name`` of ``spec_class``."""
    prefix = "Each element of the" if err.is_item else "The"
    v = err.value
    message = (
        f"{prefix} '{name}' trait of {_article(spec_class)} instance must be {err.info}, "
        f"but a value of {v!r} {type(v)} was specified."
    )
    return _Raised("traits.trait_errors.TraitError", message)


def _literal(text: str | None) -> Any:
    return UNDEFINED if text is None else ast.literal_eval(text)


def _validate(d: Mapping[str, Any], value: Any) -> Any:
    """The value a trait of description ``d`` stores for ``value`` (traits' and nipype's
    ``validate``), or :class:`_Invalid`. nipype's ``Undefined`` is always taken."""
    if value is UNDEFINED:
        return value
    k = d["k"]
    info = d["info"]
    if k == "any":
        return value
    if k == "bool":
        if isinstance(value, bool):
            return value
        raise _Invalid(info, value)
    if k == "int":
        if isinstance(value, int):
            return int(value)
        raise _Invalid(info, value)
    if k == "float":
        if isinstance(value, float):
            return value
        if isinstance(value, int):
            return float(value)
        raise _Invalid(info, value)
    if k == "range":
        return _validate_range(d, value)
    if k == "str":
        if isinstance(value, str):
            return value
        raise _Invalid(info, value)
    if k == "enum":
        for v in d["values"]:
            if _literal(v) == value:
                return value
        raise _Invalid(info, value)
    if k == "path":
        return _validate_path(d, value)
    if k == "dict":
        if isinstance(value, dict):
            return value
        raise _Invalid(info, value)
    if k == "instance":
        if value is None and d.get("allow_none"):
            return value
        mod, _, cls = d["klass"].rpartition(".")
        t = getattr(sys.modules.get(mod) or __import__(mod, fromlist=[cls]), cls)
        if isinstance(value, t):
            return value
        raise _Invalid(info, value)
    if k == "tuple":
        types = d["of"]
        if isinstance(value, tuple) and len(value) == len(types):
            try:
                return tuple(_validate(t, v) for t, v in zip(types, value, strict=True))
            except _Invalid:
                raise _Invalid(info, value) from None
        raise _Invalid(info, value)
    if k == "either":
        for alt in d["of"]:
            try:
                return _validate(alt, value)
            except _Invalid:
                continue
        raise _Invalid(info, value)
    if k == "multi":
        # nipype's MultiObject.validate
        if not isinstance(value, str | bytes) and isinstance(value, Sequence):
            value = list(value)
        if isinstance(value, list) and len(value) == 0:
            return UNDEFINED
        if not isinstance(value, list):
            value = [value]
        return _validate_list(d, value, 0, None)
    if k == "list":
        if not isinstance(value, list):
            raise _Invalid(info, value)
        return _validate_list(d, value, d["minlen"], d["maxlen"])
    raise ValueError(f"trait kind {k!r}")


def _validate_list(d: Mapping[str, Any], value: list, minlen: int, maxlen: int | None) -> list:
    if len(value) < minlen or (maxlen is not None and len(value) > maxlen):
        raise _Invalid(d["info"], value)
    out = []
    for item in value:
        try:
            out.append(_validate(d["of"], item))
        except _Invalid as e:
            raise _Invalid(d["of"]["info"] if not e.is_item else e.info, item, True) from None
    return out


def _validate_range(d: Mapping[str, Any], value: Any) -> Any:
    info = d["info"]
    if d["int"]:
        if not isinstance(value, int):
            raise _Invalid(info, value)
        value = int(value)
    else:
        if isinstance(value, int):
            value = float(value)
        elif not isinstance(value, float):
            raise _Invalid(info, value)
    low, high = d["low"], d["high"]
    if low is not None and (value < low or (d["exclude_low"] and value == low)):
        raise _Invalid(info, value)
    if high is not None and (value > high or (d["exclude_high"] and value == high)):
        raise _Invalid(info, value)
    return value


def _validate_path(d: Mapping[str, Any], value: Any) -> str:
    """nipype's ``BasePath.validate`` (its error shows the value's text)."""
    info = d["info"]
    try:
        p = Path(value)
    except TypeError:
        raise _Invalid(info, str(value)) from None
    missing = (
        not p.exists() or (d["is_file"] and not p.is_file()) or (d["is_dir"] and not p.is_dir())
    )
    if d["exists"] and missing:
        raise _Invalid(info, str(p))
    if d["resolve"]:
        p = p.resolve(strict=d["exists"]) if p.is_absolute() else (Path.cwd() / p).resolve()
    return str(p)


# --- PIPE-81 and PIPE-82: reading a text by its trait --------------------------------------------


def _from_text(d: Mapping[str, Any], value: Any) -> Any:
    """``value`` (a text, or a list of texts) read by the trait of description ``d`` (PIPE-81,
    PIPE-82): unchanged where the trait takes text or where the text reads as nothing it takes."""
    if isinstance(value, list):
        if d["k"] in ("list", "multi"):
            return [_from_text(d["of"], v) for v in value]
        return value
    if not isinstance(value, str):
        return value
    k = d["k"]
    if k == "list":
        try:
            items = ast.literal_eval(value)
        except (ValueError, SyntaxError, TypeError, MemoryError, RecursionError):
            return value
        return [_from_text(d["of"], v) for v in items] if isinstance(items, list) else value
    try:
        if k == "int" or (k == "range" and d["int"]):
            return int(value)
        if k == "float" or k == "range":
            return float(value)
    except ValueError:
        return value
    if k == "bool":
        return {"True": True, "1": True, "False": False, "0": False}.get(value, value)
    if k == "enum":
        values = [_literal(v) for v in d["values"]]
        if value in values:
            return value
        for v in values:
            if str(v) == value:
                return v
        return value
    if k == "tuple":
        try:
            t = ast.literal_eval(value)
        except (ValueError, SyntaxError, TypeError, MemoryError, RecursionError):
            return value
        return tuple(t) if isinstance(t, tuple | list) else value
    if k == "dict":
        try:
            return json.loads(value)
        except ValueError:
            return value
    if k == "either":
        for alt in d["of"]:
            v = _from_text(alt, value)
            try:
                _validate(alt, v)
            except _Invalid:
                continue
            return v
        return value
    return value


# --- the interface's inputs ----------------------------------------------------------------------


class _Inputs:
    """An interface's inputs as nipype's input spec holds them (nipype's defaults where the trait
    has ``usedefault``, else ``Undefined``)."""

    def __init__(self, spec: Mapping[str, Any]) -> None:
        self.spec = spec
        self.values: dict[str, Any] = {n: _literal(s["default"]) for n, s in spec["inputs"].items()}

    def get(self, name: str) -> Any:
        """The input's value, or ``None`` when undefined."""
        v = self.values[name]
        return None if v is UNDEFINED else v

    def defined(self, name: str) -> bool:
        return _defined(self.values[name])

    def set(self, name: str, value: Any) -> None:
        """``setattr(inputs, name, value)``: traits' validation, then nipype's ``_xor_warn`` and
        ``_deprecated_warn``."""
        s = self.spec["inputs"][name]
        try:
            value = _validate(s["validate"], value)
        except _Invalid as e:
            raise _trait_error(self.spec["spec_class"], name, e) from None
        self.values[name] = value
        if not _defined(value):
            return
        for other in s.get("xor") or ():
            if other != name and self.defined(other):
                self.values[name] = UNDEFINED
                raise _Raised(
                    "OSError",
                    f'Input "{name}" is mutually exclusive with input "{other}", which is already '
                    "set",
                )
        if s.get("deprecated"):
            self._deprecated(name, s, value)

    def _deprecated(self, name: str, s: Mapping[str, Any], value: Any) -> None:
        iface = self.spec["spec_class"].split("InputSpec")[0]
        msg1 = f"Input {name} in interface {iface} is deprecated."
        msg2 = f"Will be removed or raise an error as of release {s['deprecated']}"
        new = s.get("new_name")
        if new:
            if new not in self.spec["items"]:
                raise _Raised(
                    "traits.trait_errors.TraitError",
                    msg1 + f" Replacement trait {new} not found",
                )
            msg3 = f"It has been replaced by {new}."
        else:
            msg3 = ""
        msg = f"{msg1} {msg2} {msg3}"
        if s["deprecated_raises"]:
            raise _Raised("traits.trait_errors.TraitError", msg)
        if new:
            msg += f"Unsetting old value {name}; setting new value {new}."
        warnings.warn(msg, UserWarning, stacklevel=2)
        if new:
            self.values[name] = UNDEFINED
            self.values[new] = value

    def check_mandatory(self) -> None:
        """nipype's ``_check_mandatory_inputs`` (with ``_check_xor`` and ``_check_requires``)."""
        cls = self.spec["class"]
        for name in self.spec["check_mandatory"]:
            s = self.spec["inputs"][name]
            v = self.values[name]
            xor = s.get("xor")
            if xor and not any(self.defined(f) for f in xor) and not _defined(v):
                raise _Raised(
                    "ValueError",
                    f"{cls} requires a value for one of the inputs '{', '.join(xor)}'. "
                    f"For a list of required inputs, see {cls}.help()",
                )
            if not _defined(v) and xor is None:
                raise _Raised(
                    "ValueError",
                    f"{cls} requires a value for input '{name}'. For a list of required "
                    f"inputs, see {cls}.help()",
                )
            if _defined(v):
                self._check_requires(name, s, v)
        for name in self.spec["check_requires"]:
            self._check_requires(name, self.spec["inputs"][name], self.values[name])

    def _check_requires(self, name: str, s: Mapping[str, Any], value: Any) -> None:
        req = s.get("requires")
        if not req:
            return
        if any(not self.defined(f) for f in req) and _defined(value):
            cls = self.spec["class"]
            many = len(req) > 1
            what = "values for inputs" if many else "a value for input"
            names = ", ".join(f"'{r}'" for r in req)
            raise _Raised(
                "ValueError",
                f"{cls} requires {what} {names} because '{name}' is set. For a list of "
                f"required inputs, see {cls}.help()",
            )

    def __str__(self) -> str:
        """nipype's ``BaseTraitedSpec.__repr__``."""
        lines = [f"{n} = {self.values[n]}" for n in self.spec["printed"]]
        return "\n{}\n".format("\n".join(lines))


def _outputs_text(spec: Mapping[str, Any], outputs: Mapping[str, Any]) -> str:
    """nipype's printing of the interface's outputs (``print(res.outputs)``), with nipype's
    ``aggregate_outputs``' validation of each value."""
    described = spec["outputs"]
    if described is None:
        return "None"
    values: dict[str, Any] = {}
    for name, d in described.items():
        v = outputs.get(name, UNDEFINED)
        if v is None:
            v = UNDEFINED
        elif isinstance(v, tuple) and d["k"] in ("list", "multi"):
            v = list(v)  # larmorx's results hold tuples where nipype's hold lists
        try:
            values[name] = _validate(d, v)
        except _Invalid as e:
            if "an existing" in d["info"]:
                raise _Raised(
                    "FileNotFoundError",
                    f"No such file or directory '{v}' for output '{name}' of a "
                    f"{spec['class']} interface",
                ) from None
            raise _trait_error(spec["class"] + "OutputSpec", name, e) from None
    for name, d in described.items():
        v = values[name]
        if d.get("output_multi") and isinstance(v, list):  # OutputMultiObject's get
            values[name] = UNDEFINED if not v else v[0] if len(v) == 1 else v
    lines = [f"{n} = {values[n]}" for n in described]
    return "\n{}\n".format("\n".join(lines))


# --- the command line ----------------------------------------------------------------------------


class _UsageError(Exception):
    """A usage error of click's: ``with_usage`` when click shows the command's usage line (an
    error of a parameter's value; its parser's own errors have no context, so no usage)."""

    def __init__(self, message: str, with_usage: bool = False) -> None:
        super().__init__(message)
        self.with_usage = with_usage


def _click_args(args: Sequence[str]) -> tuple[bool, str | None, str | None, list[str]]:
    """click's parsing of ``nipypecli run [OPTIONS] [MODULE] [INTERFACE]`` with unknown options
    passed through (``--list``, and ``-h``/``--help``, a flag the command never reads): whether
    ``--list`` was given, the module, the interface and the remaining arguments
    (``ctx.args``)."""
    listed = False
    largs: list[str] = []
    rargs = list(args)
    while rargs:
        arg = rargs.pop(0)
        if arg == "--":
            break
        if arg[:1] == "-" and len(arg) > 1:
            long_opt, sep, _value = arg.partition("=")
            if long_opt in ("--list", "--help"):
                if sep:
                    raise _UsageError(f"Option '{long_opt}' does not take a value.")
                listed = listed or long_opt == "--list"
            elif arg[:2] == "--":
                largs.append(arg)
            else:
                unknown = [ch for ch in arg[1:] if ch != "h"]
                if unknown:
                    largs.append("-" + "".join(unknown))
        else:
            largs.append(arg)
    rest = largs + rargs
    module = rest[0] if rest else None
    interface = rest[1] if len(rest) > 1 else None
    return listed, module, interface, rest[2:]


_TYPES: dict[str | None, Callable[[str], Any] | None] = {
    None: None,
    "str": str,
    "int": int,
    "float": float,
    "bool": bool,
    "json": json.loads,
}


def _parser(
    prog: str, interface: str, calls: Sequence[Mapping[str, Any]]
) -> argparse.ArgumentParser:
    """nipype's interface parser: ``add_args_options``' ``add_argument`` calls."""
    parser = argparse.ArgumentParser(description=f"Run {interface}", prog=prog)
    for c in calls:
        kwargs: dict[str, Any] = {}
        for k, v in c.items():
            if k in ("args", "default_undefined"):
                continue
            if k == "type":
                kwargs["type"] = _TYPES[v]
            elif k == "default":
                kwargs["default"] = UNDEFINED if c.get("default_undefined") else v
            else:
                kwargs[k] = v
        parser.add_argument(*c["args"], **kwargs)
    return parser


def _threads(n_threads: int | None, env: Mapping[str, str]) -> int:
    if n_threads is None:
        text = env.get("OMP_NUM_THREADS", "").strip()
        n_threads = int(text) if text.isdigit() else 0
    return n_threads if n_threads > 0 else (os.cpu_count() or 1)


def run(
    argv: Sequence[str] | None = None,
    *,
    prog: str = "larmorx nipype run",
    upstream_defects: str | None = None,
    n_threads: int | None = None,
    env: Mapping[str, str] | None = None,
) -> int:
    """``nipypecli run`` with ``argv`` (its arguments: ``[--list] <module> [<Interface>
    [inputs]]``; by default ``sys.argv[1:]``) and return its exit status: 0, 2 for a usage error
    (click's or argparse's), 1 for an error (nipype's traceback as its first and last lines on
    standard error). The interface runs in the current folder, which receives its files.
    ``prog`` is the command's name in its messages (nipype's ``nipypecli run``);
    ``upstream_defects`` defaults to ``LARMORX_UPSTREAM_DEFECTS`` (else ``"fix"``),
    ``n_threads`` to ``OMP_NUM_THREADS`` (else every logical CPU; the results do not depend on
    it); ``env`` is the environment read (by default ``os.environ``)."""
    from larmorx import _defects

    argv = sys.argv[1:] if argv is None else list(argv)
    env = os.environ if env is None else env
    mode = upstream_defects or env.get(_DEFECTS_ENV, "") or "fix"
    if mode not in _defects.UPSTREAM_DEFECTS:
        sys.stderr.write(f"error: {_DEFECTS_ENV} must be 'fix' or 'reproduce', not {mode!r}\n")
        return 2
    try:
        with _as_nipype():
            return _main(argv, prog, mode, _threads(n_threads, env))
    except _Raised as e:
        sys.stdout.flush()
        sys.stderr.write(_traceback(e.name, e.message))
        return 1
    except SystemExit as e:
        return e.code if isinstance(e.code, int) else (0 if e.code is None else 1)
    except Exception as e:
        sys.stdout.flush()
        sys.stderr.write(_traceback(_type_name(type(e)), str(e)))
        return 1


def _main(argv: list[str], prog: str, mode: str, n_threads: int) -> int:
    usage = f"Usage: {prog} [OPTIONS] [MODULE] [INTERFACE]\n"
    try:
        listed, module, interface, extra = _click_args(argv)
        if module is None:
            raise _UsageError("Invalid value for '[MODULE]': got None.", with_usage=True)
    except _UsageError as e:
        sys.stderr.write((f"{usage}\n" if e.with_usage else "") + f"Error: {e}\n")
        return 2
    if module not in MODULES:
        sys.stderr.write(f"error: {_module_message(module)}\n")
        return 1
    data = _data()["modules"][module]
    if listed or not interface:
        print("Available Interfaces:")
        for name in data["interfaces"]:
            print(f"    {name}")
        return 0
    spec = data["specs"].get(interface)
    if spec is None:
        raise _Raised("AttributeError", f"module '{module}' has no attribute '{interface}'")
    if "init_error" in spec:
        raise _Raised(spec["init_error"]["type"], spec["init_error"]["message"])
    corrected = mode == "fix" and "calls_corrected" in spec
    if corrected and "calls" not in spec:
        from larmorx import _defects

        _defects.warn(
            [
                (
                    "PIPE-81",
                    "nipype command line",
                    "fix",
                    f"{spec['class']}: nipype's command line cannot build this interface's parser "
                    f"({spec['parser_error']['type']}: {spec['parser_error']['message']}); "
                    "larmorx builds it, its compound inputs read by their traits",
                )
            ]
        )
    texts: dict[str, str | None] = {}
    if corrected:
        calls, texts = _text_calls(spec)
    elif "calls" in spec:
        calls = spec["calls"]
    else:
        raise _Raised(spec["parser_error"]["type"], spec["parser_error"]["message"])
    parser = _parser(" ".join([prog, module, interface, *extra]), interface, calls)
    if not extra:
        try:
            parser.print_help()
        except Exception:
            print("An error occurred when trying to print the fullcommand help, printing usage.")
        finally:
            parser.print_usage()
        return 0
    options = parser.parse_args(args=extra)
    from larmorx.nipype._adapters import ADAPTERS

    adapter = ADAPTERS.get((module, interface))
    if adapter is None:
        sys.stderr.write(
            f"error: larmorx has not ported {module}.{interface} (larmorx nipype run lists it "
            "and prints its help as nipype does; the interfaces it runs: "
            "lx.nipype.ported_interfaces())\n"
        )
        return 1
    inputs = _set_inputs(spec, parser, options, mode, texts)
    print(inputs)
    inputs.check_mandatory()
    outputs = adapter(inputs, _Context(os.getcwd(), mode, n_threads))
    print(_outputs_text(spec, outputs))
    return 0


def _set_inputs(
    spec: Mapping[str, Any],
    parser: argparse.ArgumentParser,
    options: argparse.Namespace,
    mode: str,
    texts: Mapping[str, str | None],
) -> _Inputs:
    """nipype's ``run_instance`` up to the printing of the inputs: each parsed value set on the
    inputs in name order (a value nipype's ``ValueError`` refuses is reported and skipped).
    ``texts``: the inputs parsed as text (PIPE-81, PIPE-82), with the argparse type of the
    corrected parser, read here."""
    if mode == "fix":  # the texts read first: a text argparse would refuse is its error
        for name in spec["items"]:
            value = getattr(options, name, None)
            if value is not None:
                setattr(options, name, _corrected_value(spec, parser, name, value, texts))
    print("setting function inputs")
    inputs = _Inputs(spec)
    for name in spec["items"]:
        if not hasattr(options, name):
            raise _Raised("AttributeError", f"'Namespace' object has no attribute '{name}'")
        value = getattr(options, name)
        if value is None:
            continue
        try:
            inputs.set(name, value)
        except ValueError as e:  # nipype catches these only (traits' errors are not ValueErrors)
            print(f"Error when setting the value of {name}: '{e}'")
    return inputs


def _types(calls: Sequence[Mapping[str, Any]]) -> dict[str, str | None]:
    return {c["args"][0].lstrip("-"): c.get("type") for c in calls}


def _text_calls(spec: Mapping[str, Any]) -> tuple[list[dict[str, Any]], dict[str, str | None]]:
    """The corrected parser's calls, with no argparse type where its type differs from nipype's
    (or nipype has no parser): those inputs are parsed as text, and read by
    :func:`_corrected_value`; the corrected types of those inputs."""
    nipype = _types(spec.get("calls", []))
    calls, texts = [], {}
    for c in spec["calls_corrected"]:
        name = c["args"][0].lstrip("-")
        if "calls" not in spec or nipype.get(name) != c.get("type"):
            texts[name] = c.get("type")
            c = {k: v for k, v in c.items() if k != "type"}
        calls.append(dict(c))
    return calls, texts


def _convert(type_name: str | None, value: Any) -> Any:
    """argparse's conversion of the text(s) ``value`` by ``type_name`` (``ValueError``)."""
    convert = _TYPES[type_name]
    if convert is None:
        return value
    if isinstance(value, list):
        return [convert(v) for v in value]
    return convert(value)


def _corrected_value(
    spec: Mapping[str, Any],
    parser: argparse.ArgumentParser,
    name: str,
    value: Any,
    texts: Mapping[str, str | None],
) -> Any:
    """PIPE-81 and PIPE-82: ``value`` read as its trait reads it, with a notice where nipype's
    command line would have given the trait another value, or refused it."""
    from larmorx import _defects

    d = spec["inputs"][name]["validate"]
    pipe81 = False
    if name in texts:
        wanted = texts[name]
        try:
            converted = _convert(wanted, value)
        except (ValueError, TypeError):
            bad = value
            if isinstance(value, list):
                bad = next(v for v in value if _fails(wanted, v))
            option = name if spec["inputs"][name]["mandatory"] else f"--{name}"
            parser.error(f"argument {option}: invalid {wanted} value: {bad!r}")
        read = _from_text(d, converted)
        nipype_type = _types(spec.get("calls", [])).get(name)
        pipe81 = spec["inputs"][name]["validate"]["k"] == "either" or (
            spec["inputs"][name]["validate"]["k"] in ("list", "multi")
            and spec["inputs"][name]["validate"]["of"]["k"] == "either"
        )
    else:
        read = _from_text(d, value)
        nipype_type = None
        if read is value or _same(read, value):
            return value
    try:
        ours = _validate(d, read)
    except _Invalid:
        return read  # refused by larmorx too: the trait's error follows
    if "calls" not in spec:
        return read  # nipype has no parser: said when larmorx built it
    try:
        nipype = _validate(d, _convert(nipype_type, value))
    except (ValueError, TypeError, _Invalid):
        nipype = None
    if nipype is not None and _same(nipype, ours):
        return read
    if pipe81:
        entry = "PIPE-81"
        detail = (
            f"{spec['class']}: nipype's command line reads {name!r} as "
            f"{nipype_type or 'text'} (the type of the input before it in name order); larmorx "
            "reads it by its trait"
        )
    else:
        entry = "PIPE-82"
        detail = (
            f"{spec['class']}: nipype's command line reads {name!r} as "
            f"{nipype_type or 'text'}, not as its trait reads it; larmorx reads it by its trait"
        )
    _defects.warn([(entry, "nipype command line", "fix", detail)], stacklevel=3)
    return read


def _fails(type_name: str | None, text: Any) -> bool:
    try:
        _convert(type_name, text)
    except (ValueError, TypeError):
        return True
    return False


def _same(a: Any, b: Any) -> bool:
    """Equal values of the same types (``1 == 1.0 == True`` are not the same input)."""
    if isinstance(a, list | tuple) and isinstance(b, list | tuple):
        return type(a) is type(b) and len(a) == len(b) and all(map(_same, a, b))
    return type(a) is type(b) and a == b


class _Context:
    """How an interface runs: its folder (nipype's ``runtime.cwd``), D13's mode, the threads."""

    def __init__(self, cwd: str, mode: str, n_threads: int) -> None:
        self.cwd, self.mode, self.n_threads = cwd, mode, n_threads


class _NipypeFormatter(logging.Formatter):
    """nipype's log lines (``utils/logger.py``): ``%y%m%d-%H:%M:%S,<ms> nipype.interface LEVEL:``
    and the message on the next line after a tab."""

    def __init__(self) -> None:
        super().__init__(
            fmt="%(asctime)s,%(msecs)d nipype.interface %(levelname)-2s:\n\t %(message)s",
            datefmt="%y%m%d-%H:%M:%S",
        )


@contextmanager
def _as_nipype() -> Iterator[None]:
    """nipype's logging (its interfaces' messages at INFO and above on standard output) and
    warnings (Python's warnings not shown, larmorx's notices of corrected or reproduced defects
    shown)."""
    from larmorx import _defects

    handler = logging.StreamHandler(sys.stdout)
    handler.setFormatter(_NipypeFormatter())
    loggers = [logging.getLogger(n) for n in ("larmorx.confounds", "larmorx.niworkflows")]
    saved = [(lg.level, lg.propagate) for lg in loggers]
    for lg in loggers:
        lg.addHandler(handler)
        lg.setLevel(logging.INFO)
        lg.propagate = False
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            warnings.filterwarnings("default", category=_defects.UpstreamDefectWarning)
            yield
    finally:
        for lg, (level, propagate) in zip(loggers, saved, strict=True):
            lg.removeHandler(handler)
            lg.setLevel(level)
            lg.propagate = propagate
