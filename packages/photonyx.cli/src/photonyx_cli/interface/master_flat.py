import cappa
import pathlib
import structlog
import typing as t
from dataclasses import dataclass

log = structlog.stdlib.get_logger()


@cappa.command(name="master-flat", invoke="photonyx_cli.command.master_flat.invoke")
@dataclass
class MasterFlatCommand:
    """Create Calibration Master Flat."""

    input: t.Annotated[pathlib.Path, cappa.Arg(help="Path to the RAW folder")]

    # TODO: get from hardware if using the standard dir layout
    output: t.Annotated[pathlib.Path, cappa.Arg(help="Path to save master")]
    
    # TODO: get this from hardware profile that matches closest to the date of the flat
    bias: t.Annotated[pathlib.Path, cappa.Arg(help="Path to Master BIAS")]
    
    # TODO: make this smart (but ASIAir images don't have filter name in fits header)
    filter: t.Annotated[t.Optional[str], cappa.Arg(help="The name of the filter for the master flat", short=True)] = None

    def __post_init__(self):
        log.debug("Master Flat command initialized", input=self.input, output=self.output, bias=self.bias)
        pass
