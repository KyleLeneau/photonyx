from __future__ import annotations
import cappa
import structlog
import asyncio
import pathlib

from ..interface.app import PhotonyxApp
from ..interface.stack import StackCommand
from ..utils.file_watcher import watch_files

from async_siril import SirilCli
from async_siril.command import set32bits, setext, start_ls, stop_ls, livestack
from async_siril.command_types import fits_extension

from rich.prompt import Confirm


log = structlog.stdlib.get_logger()


async def invoke(app: PhotonyxApp, command: StackCommand, output: cappa.Output):
    log.info(app)
    log.info(command)
    output("Stacking not implemented yet")

    Confirm.ask("Continue")
