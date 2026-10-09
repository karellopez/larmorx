"""Type stubs for the compiled extension module (crate ``larmorx-py``)."""

__version__: str

def cli_main(argv: list[str]) -> tuple[int, str, str]:
    """Run the ``larmorx`` command line; return ``(exit_code, stdout, stderr)``."""
