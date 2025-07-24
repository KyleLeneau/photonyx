from __future__ import annotations
import cappa
import asyncio
import structlog

from photonyx.interface.app import PhotonyxApp

log = structlog.get_logger()


def main() -> None:  # pragma: no cover
    try:
        asyncio.run(cappa.invoke_async(PhotonyxApp))
    except Exception as e:
        log.exception("Unhandled exception")
        raise cappa.Exit("There was an error while executing", code=-1) from e


if __name__ == "__main__":
    main()
