from dataclasses import dataclass
import cappa
import pathlib
import typing as t
import structlog

log = structlog.stdlib.get_logger()


@cappa.command(name="preprocess", invoke="photonyx_cli.command.preprocess.invoke")
@dataclass
class PreprocessCommand:
    """Start a calibration session for a target."""

    folder: t.Annotated[pathlib.Path, cappa.Arg(help="Path to the session folder")]

    def __post_init__(self):
        # log.debug("Calibration command initialized", folder=self.folder)
        pass
