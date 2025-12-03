from __future__ import annotations
import cappa
import structlog

from ..interface.app import PhotonyxApp
from ..interface.master_dark import MasterDarkCommand
from photonyx_engine.master_dark import create_calibration_master_dark
from async_siril.command_types import fits_extension

log = structlog.stdlib.get_logger()


async def invoke(app: PhotonyxApp, command: MasterDarkCommand, output: cappa.Output):
    try:
        master = await create_calibration_master_dark(
            raw_folder=command.input.resolve(),
            output_folder=command.output.resolve(),
            extension=fits_extension.FITS_EXT_FITS,
        )
        output.output(f"Master DARK created: {master}")
    except Exception as e:
        log.error(str(e))
        output.error(e)
