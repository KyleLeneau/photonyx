import asyncio
import pathlib
from photonyx_engine.master_dark import create_calibration_master_dark
from async_siril.command_types import fits_extension


async def _test_create_master_dark():
    input = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/DARK/2025-11-04/O-1x1/120")
    output = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/DARK/2025-11-04")

    await create_calibration_master_dark(raw_folder=input, output_folder=output, extension=fits_extension.FITS_EXT_FITS)


if __name__ == "__main__":
    asyncio.run(_test_create_master_dark())
