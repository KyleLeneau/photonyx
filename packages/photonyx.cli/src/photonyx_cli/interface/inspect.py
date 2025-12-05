import cappa
import pathlib
import typing as t
from dataclasses import dataclass


@cappa.command(name="inspect", invoke="photonyx_cli.command.inspect.invoke")
@dataclass
class InspectCommand:
    """Inspect a single fits file."""

    file: t.Annotated[pathlib.Path, cappa.Arg(help="Path to the fit/fits file to inspect")]
