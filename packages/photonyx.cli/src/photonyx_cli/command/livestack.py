from __future__ import annotations
import cappa
import structlog

from ..interface.app import PhotonyxApp
from ..interface.livestack import LivestackCommand


from rich.prompt import Confirm


log = structlog.stdlib.get_logger()


async def invoke(app: PhotonyxApp, command: LivestackCommand, output: cappa.Output):
    log.info(app)
    log.info(command)
    output("Livestack not implemented yet")

    Confirm.ask("Continue")
