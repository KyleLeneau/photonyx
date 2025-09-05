from dataclasses import dataclass
import cappa
import pathlib
import typing as t
import structlog

log = structlog.stdlib.get_logger()


@cappa.command(name="livestack", invoke="photonyx_cli.command.livestack.invoke")
@dataclass
class LivestackCommand:
    """Start a livestacking session for a project."""

    folder: t.Annotated[pathlib.Path, cappa.Arg(help="Path to the project folder")]

    def __post_init__(self):
        log.info("Livestack command initialized", folder=self.folder)
        # TODO: load and merge all config files from this folder up to the root
