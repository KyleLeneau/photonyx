import asyncio
import pathlib
from photonyx_engine.master_bias import create_calibration_master_bias
from async_siril.command_types import fits_extension


async def _test_create_master_bias():
    input = pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/BIAS/2025-06-24/RAW")
    output = pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/BIAS/2025-06-24")

    await create_calibration_master_bias(raw_folder=input, output_folder=output, extension=fits_extension.FITS_EXT_FIT)


if __name__ == "__main__":
    asyncio.run(_test_create_master_bias())
