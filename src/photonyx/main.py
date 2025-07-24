from __future__ import annotations
from dataclasses import dataclass
import cappa
import asyncio
import structlog

log = structlog.get_logger()


@dataclass
class PhotonyxApp:
    """Photonyx application."""

    commands: cappa.Subcommands[VersionCommand]


@cappa.command(name="version")
@dataclass
class VersionCommand:
    """Show the version of Photonyx."""

    def __call__(self, output: cappa.Output):
        output("Photonyx version 0.1.0")


def main() -> None:  # pragma: no cover
    try:
        asyncio.run(cappa.invoke_async(PhotonyxApp))
    except Exception as e:
        log.exception("Unhandled exception")
        raise cappa.Exit("There was an error while executing", code=-1) from e


if __name__ == "__main__":
    main()
