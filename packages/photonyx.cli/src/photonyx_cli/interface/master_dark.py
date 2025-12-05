import cappa
import pathlib
import structlog
import typing as t
from dataclasses import dataclass
from . import BaseProfileCommand

log = structlog.stdlib.get_logger()


@cappa.command(name="master-dark", invoke="photonyx_cli.command.master_dark.invoke")
@dataclass
class MasterDarkCommand(BaseProfileCommand):
    """Create a Master DARK frame for preprocessing LIGHT frames."""

    input: t.Annotated[pathlib.Path, cappa.Arg(help="Path to the RAW folder")]

    output: t.Annotated[t.Optional[pathlib.Path], cappa.Arg(help="Path to save master")] = None

    def __post_init__(self):
        self.load_profile(self.input)
        self.output = self.resolve_output(self.output, "dark")
        log.debug("Master Dark command initialized", input=self.input, output=self.output)
