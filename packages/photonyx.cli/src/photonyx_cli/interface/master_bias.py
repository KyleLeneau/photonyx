import cappa
import pathlib
import structlog
import typing as t
from dataclasses import dataclass

log = structlog.stdlib.get_logger()


@cappa.command(name="master-bias", invoke="photonyx_cli.command.master_bias.invoke")
@dataclass
class MasterBiasCommand:
    """Create a Master BIAS used for preprocessing LIGHT frames."""

    input: t.Annotated[pathlib.Path, cappa.Arg(help="Path to the RAW folder")]

    # TODO: get from hardware if using the standard dir layout
    output: t.Annotated[pathlib.Path, cappa.Arg(help="Path to save master")]

    def __post_init__(self):
        log.debug("Master Bias command initialized", input=self.input, output=self.output)
        pass
