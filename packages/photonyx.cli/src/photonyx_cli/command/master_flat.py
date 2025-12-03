from __future__ import annotations
import cappa
import structlog

from ..interface.app import PhotonyxApp
from ..interface.master_flat import MasterFlatCommand
from photonyx_engine.master_flat import create_calibration_master_flat
from async_siril.command_types import fits_extension

log = structlog.stdlib.get_logger()


async def invoke(app: PhotonyxApp, command: MasterFlatCommand, output: cappa.Output):
    try:
        master = await create_calibration_master_flat(
            raw_folder=command.input.resolve(),
            output_folder=command.output.resolve(),
            master_bias=command.bias.resolve(),
            filter_name=command.filter,
            extension=fits_extension.FITS_EXT_FITS,
        )
        output.output(f"Master FLAT created: {master}")
    except Exception as e:
        log.error(str(e))
        output.error(e)
