from dataclasses import dataclass
import cappa
import pathlib
import typing as t
import structlog

log = structlog.stdlib.get_logger()


@cappa.command(name="stack", invoke="photonyx_cli.command.stack.invoke")
@dataclass
class StackCommand:
    """Start a stacking session for a given project."""

    folder: t.Annotated[pathlib.Path, cappa.Arg(help="Path to the session folder")]

    def __post_init__(self):
        log.info("Stack command initialized", folder=self.folder)
        # TODO: load and merge all config files from this folder up to the root
