import asyncio
import pathlib
from photonyx_engine.master_dark import create_calibration_master_dark
from async_siril.command_types import fits_extension


async def _test_create_master_dark():
    input = pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/DARK/2025-06-24/RAW_300")
    output = pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/DARK/2025-06-24")

    await create_calibration_master_dark(
        raw_folder=input, output_folder=output, name_suffix="300s_-10C", extension=fits_extension.FITS_EXT_FIT
    )


if __name__ == "__main__":
    asyncio.run(_test_create_master_dark())
