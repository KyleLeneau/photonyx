from __future__ import annotations
import cappa
import structlog

from ..interface.app import PhotonyxApp
from ..interface.master_bias import MasterBiasCommand
from photonyx_engine.master_bias import create_calibration_master_bias
from async_siril.command_types import fits_extension

log = structlog.stdlib.get_logger()


async def invoke(app: PhotonyxApp, command: MasterBiasCommand, output: cappa.Output):
    try:
        master = await create_calibration_master_bias(
            raw_folder=command.input.resolve(),
            output_folder=command.output.resolve(),
            extension=fits_extension.FITS_EXT_FITS,
        )
        output.output(f"Master BIAS created: {master}")
    except Exception as e:
        log.error(e)
        output.error(e)
