# SPDX-License-Identifier: Apache-2.0
"""``larmorx nipype run``'s interface descriptions, read from nipype's command line itself.

``nipypecli run <module> <Interface>`` (nipype 1.12.0, ``nipype/scripts/cli.py``) builds an
argparse parser from the interface's input traits (``scripts/utils.add_args_options``), sets the
parsed values on the inputs (``utils/nipype_cmd.run_instance``), prints them, runs the interface
and prints its outputs. larmorx runs the interfaces it has ported without nipype or traits, so it
needs, for every interface of the modules it serves, what nipype's command line derives from the
traits: the ``add_argument`` calls exactly as nipype makes them (recorded here by wrapping
``ArgumentParser.add_argument``: the help texts are nipype's ``get_trait_desc``), the inputs'
defaults as nipype prints them, each trait's validation (its kind, bounds, values, the text of
traits' error message), the ``xor``, ``requires`` and ``deprecated`` metadata, the outputs, and
the errors nipype's command line meets (an interface it cannot instantiate, or whose parser it
cannot build: ``DictMerge``, ``TSV2JSON``, fMRIPrep's ``FMRISummary``).

Run it in the newest environment (nipype 1.12.0, niworkflows 1.15.4, fMRIPrep 25.2.7 from
``<workspace>/tags/fmriprep-25.2.7`` on ``PYTHONPATH``, traits 7.2.0)::

    PYTHONPATH=<workspace>/tags/fmriprep-25.2.7 NIPYPE_NO_ET=1 \\
        <workspace>/oracles/python-sdcflows-2.17/bin/python -P \\
        validation/investigations/confounds/nipype_cli_spec.py [--check]

It writes ``python/larmorx/nipype/interfaces.json`` (``--check``: compares with it and exits 1 on
a difference). The data are nipype's, niworkflows' and fMRIPrep's (Apache-2.0).
"""

from __future__ import annotations

import argparse
import importlib
import json
import sys
import warnings
from pathlib import Path

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

OUT = Path(__file__).resolve().parents[3] / "python" / "larmorx" / "nipype" / "interfaces.json"


def _exc(e: BaseException) -> dict:
    t = type(e)
    mod = t.__module__
    name = t.__qualname__ if mod in ("builtins", "__main__") else f"{mod}.{t.__qualname__}"
    return {"type": name, "message": str(e)}


def _literal(value):
    """A value as ``ast.literal_eval`` reads it back, or ``None`` for nipype's ``Undefined``."""
    from nipype.interfaces.base import isdefined

    if not isdefined(value):
        return None
    text = repr(value)
    import ast

    assert ast.literal_eval(text) == value, text
    return text


def _type_name(t) -> str | None:
    if t is None:
        return None
    if t in (str, int, float, bool):
        return t.__name__
    if t is json.loads:
        return "json"
    raise ValueError(f"argparse type {t!r}")


def describe(handler, obj=None, name=None) -> dict:
    """A trait handler's validation: its kind and parameters, and traits' text for it."""
    import traits.api as tr
    from nipype.interfaces.base import traits_extension as te
    from traits import trait_handlers as th
    from traits import trait_types as tt

    info = handler.full_info(obj, name, None) if obj is not None else handler.info()
    d: dict = {"info": info}
    if isinstance(handler, te.MultiObject):
        d.update(k="multi", of=describe(handler.inner_traits()[0].handler))
        if isinstance(handler, te.OutputMultiObject):
            d["output_multi"] = True
    elif isinstance(handler, tt.List):
        d.update(
            k="list",
            of=describe(handler.inner_traits()[0].handler),
            minlen=handler.minlen,
            maxlen=handler.maxlen if handler.maxlen < sys.maxsize else None,
        )
    elif isinstance(handler, te.BasePath):
        d.update(
            k="path",
            exists=bool(handler.exists),
            is_file=bool(handler._is_file),
            is_dir=bool(handler._is_dir),
            resolve=bool(handler.resolve),
        )
        exts = getattr(handler, "_exts", None)
        if exts:
            d["exts"] = sorted(exts)
    elif isinstance(handler, tt.BaseTuple) or isinstance(handler, tt.Tuple):
        d.update(k="tuple", of=[describe(t.handler) for t in handler.types])
    elif isinstance(handler, tt.BaseRange):
        d.update(
            k="range",
            int=handler._validate.endswith("int") or isinstance(handler._low, int)
            and isinstance(handler._high, int | type(None)),
            low=handler._low,
            high=handler._high,
            exclude_low=bool(handler._exclude_low),
            exclude_high=bool(handler._exclude_high),
        )
    elif isinstance(handler, tt.BaseEnum | th.TraitEnum):
        values = list(handler.values)
        d.update(k="enum", values=[_literal(v) for v in values])
    elif isinstance(handler, tt.BaseBool):
        d.update(k="bool")
    elif isinstance(handler, tt.BaseInt):
        d.update(k="int")
    elif isinstance(handler, tt.BaseFloat):
        d.update(k="float")
    elif isinstance(handler, tt.BaseStr):
        d.update(k="str")
    elif isinstance(handler, tt.Dict):
        d.update(k="dict")
    elif isinstance(handler, th.TraitCompound):
        d.update(k="either", of=[describe(h) for h in handler.handlers])
    elif isinstance(handler, tt.Any):
        d.update(k="any")
    elif isinstance(handler, tr.Instance | tt.BaseInstance):
        d.update(
            k="instance",
            klass=f"{handler.klass.__module__}.{handler.klass.__qualname__}",
            allow_none=bool(handler.allow_none),
        )
    else:
        raise ValueError(f"trait handler {type(handler)}")
    return d


def add_args_options_corrected(arg_parser, interface):
    """nipype 1.12.0's ``scripts/utils.add_args_options`` (Apache-2.0, the nipype developers)
    with PIPE-81 and PIPE-82 corrected, as larmorx builds the parser by default: a compound
    input (``traits.Either``), or a list of compound items, gets no argparse type (nipype gives
    it the type of the input before it in name order, a variable left from the loop's previous
    iteration, or fails with ``UnboundLocalError`` or ``AttributeError``), and a list of
    booleans none either (nipype's ``type=bool`` makes every text but the empty one true):
    larmorx converts these texts by the trait itself. Everything else as nipype's."""
    from nipype.interfaces.base import InputMultiPath, traits
    from nipype.interfaces.base.support import get_trait_desc

    inputs = interface.input_spec()
    for name, spec in sorted(interface.inputs.traits(transient=None).items()):
        desc = "\n".join(get_trait_desc(inputs, name, spec))[len(name) + 2 :]
        desc = desc.replace("%", "%%")
        args = {}
        has_multiple_inner_traits = False

        if spec.is_trait_type(traits.Bool):
            args["default"] = getattr(inputs, name)
            args["action"] = "store_true"

        trait_type = None
        if not spec.inner_traits:
            if not spec.is_trait_type(traits.TraitCompound):
                trait_type = type(spec.trait_type.default_value)
            if trait_type in (bytes, str, int, float):
                if trait_type is bytes:
                    trait_type = str
                args["type"] = trait_type
        elif len(spec.inner_traits) == 1:
            inner = spec.inner_traits[0].trait_type
            if not isinstance(inner, traits.TraitCompound):
                trait_type = type(inner.default_value)
            if trait_type is bytes:
                trait_type = str
            if trait_type in (bytes, str, int, float):
                args["type"] = trait_type
        else:
            if len(spec.inner_traits) > 1:
                if not spec.is_trait_type(traits.Dict):
                    has_multiple_inner_traits = True

        if getattr(spec, "mandatory", False):
            if spec.is_trait_type(InputMultiPath):
                args["nargs"] = "+"
            elif spec.is_trait_type(traits.List):
                if (
                    spec.trait_type.minlen == spec.trait_type.maxlen
                ) and spec.trait_type.maxlen:
                    args["nargs"] = spec.trait_type.maxlen
                else:
                    args["nargs"] = "+"
            elif spec.is_trait_type(traits.Dict):
                args["type"] = json.loads

            if has_multiple_inner_traits:
                raise NotImplementedError(
                    "This interface cannot be used. via the"
                    " command line as multiple inner traits"
                    " are currently not supported for mandatory"
                    " argument: {}.".format(name)
                )
            arg_parser.add_argument(name, help=desc, **args)
        else:
            if spec.is_trait_type(InputMultiPath):
                args["nargs"] = "*"
            elif spec.is_trait_type(traits.List):
                if (
                    spec.trait_type.minlen == spec.trait_type.maxlen
                ) and spec.trait_type.maxlen:
                    args["nargs"] = spec.trait_type.maxlen
                else:
                    args["nargs"] = "*"
            if not has_multiple_inner_traits:
                arg_parser.add_argument("--%s" % name, dest=name, help=desc, **args)

    return arg_parser


def _recorded(build, node) -> tuple[list[dict], dict | None]:
    """The ``add_argument`` calls ``build(parser, node)`` makes, and its error."""
    from nipype.interfaces.base import Undefined

    calls: list[dict] = []
    real = argparse.ArgumentParser.add_argument

    def record(self, *args, **kwargs):
        call = {"args": list(args)}
        for k, v in kwargs.items():
            if k == "type":
                call["type"] = _type_name(v)
            elif k == "default":
                call["default"] = None if v is Undefined else v
                call["default_undefined"] = v is Undefined
            else:
                call[k] = v
        calls.append(call)
        return real(self, *args, **kwargs)

    parser = argparse.ArgumentParser(prog="x")
    argparse.ArgumentParser.add_argument = record  # type: ignore[method-assign]
    error = None
    try:
        build(parser, node)
    except Exception as e:  # noqa: BLE001 (recorded: nipype's command line meets it)
        error = _exc(e)
    finally:
        argparse.ArgumentParser.add_argument = real  # type: ignore[method-assign]
    return calls, error


def interface_spec(module, iname: str) -> dict:
    from nipype.interfaces.base import isdefined
    from nipype.scripts.utils import add_args_options

    out: dict = {"name": iname}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            node = getattr(module, iname)()
        except Exception as e:  # noqa: BLE001 (recorded: nipype's command line meets it)
            out["init_error"] = _exc(e)
            return out
    calls, error = _recorded(add_args_options, node)
    if error is None:
        out["calls"] = calls
    else:
        out["parser_error"] = error
        out["calls_before_error"] = calls
    fixed, error = _recorded(add_args_options_corrected, node)
    assert error is None, (iname, error)
    if fixed != out.get("calls"):
        out["calls_corrected"] = fixed
        if "calls" in out:  # the corrections change the types only
            strip = [{k: v for k, v in c.items() if k != "type"} for c in out["calls"]]
            assert strip == [{k: v for k, v in c.items() if k != "type"} for c in fixed], iname
    inputs = node.inputs
    out["spec_class"] = type(inputs).__name__
    out["class"] = type(node).__name__
    out["package_version"] = str(getattr(node.inputs, "package_version", ""))
    items = [n for n, _ in inputs.items()]
    out["items"] = items
    out["printed"] = sorted(inputs.trait_get())
    out["check_mandatory"] = list(inputs.traits(mandatory=True))
    out["check_requires"] = list(inputs.traits(mandatory=None, transient=None))
    from packaging.version import Version

    specs = {}
    for n in sorted(set(items) | set(out["printed"])):
        spec = inputs.traits()[n]
        value = getattr(inputs, n)
        s = {
            "printed": f"{value}",
            "default": _literal(value),
            "mandatory": bool(spec.mandatory),
            "validate": describe(spec.handler, inputs, n),
        }
        if spec.xor:
            s["xor"] = list(spec.xor)
        if spec.requires:
            s["requires"] = list(spec.requires)
        if spec.deprecated:
            s["deprecated"] = str(spec.deprecated)
            s["new_name"] = spec.new_name
            s["deprecated_raises"] = Version(str(spec.deprecated)) < inputs.package_version
        specs[n] = s
        assert isdefined(value) == (s["default"] is not None)
    out["inputs"] = specs
    outputs = node._outputs()
    out["outputs"] = (
        None
        if outputs is None
        else {
            n: describe(outputs.traits()[n].handler, outputs, n)
            for n in sorted(outputs.trait_get())
        }
    )
    out["output_class"] = type(node).__name__
    return out


def build() -> dict:
    import fmriprep
    import nipype
    import niworkflows
    import traits
    from nipype.scripts.instance import list_interfaces

    data: dict = {
        "versions": {
            "nipype": nipype.__version__,
            "niworkflows": niworkflows.__version__,
            "fmriprep": fmriprep.__version__,
            "traits": traits.__version__,
        },
        "modules": {},
    }
    for m in MODULES:
        mod = importlib.import_module(m)
        names = list_interfaces(mod)
        data["modules"][m] = {
            "interfaces": names,
            "specs": {n: interface_spec(mod, n) for n in names},
        }
    return data


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--check", action="store_true", help="compare with the committed file")
    a = ap.parse_args()
    text = json.dumps(build(), indent=1, sort_keys=True, ensure_ascii=False) + "\n"
    if a.check:
        same = OUT.read_text(encoding="utf-8") == text
        print("interfaces.json", "identical" if same else "DIFFERS")
        return 0 if same else 1
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {OUT.name}: {sum(len(v['interfaces']) for v in json.loads(text)['modules'].values())} interfaces")
    return 0


if __name__ == "__main__":
    sys.exit(main())
