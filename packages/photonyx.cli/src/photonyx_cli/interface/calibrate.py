from dataclasses import dataclass
import cappa
import pathlib
import typing as t
import structlog

log = structlog.stdlib.get_logger()


@cappa.command(name="calibrate", invoke="photonyx_cli.command.calibrate.invoke")
@dataclass
class CalibrateCommand:
    """Start a calibration session for a target."""

    folder: t.Annotated[pathlib.Path, cappa.Arg(help="Path to the session folder")]

    def __post_init__(self):
        log.info("Calibration command initialized", folder=self.folder)
        # TODO: load and merge all config files from this folder up to the root
