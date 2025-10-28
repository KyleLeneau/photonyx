import asyncio
import pathlib
from photonyx_engine.master_flat import create_calibration_master_flat
from async_siril.command_types import fits_extension


async def _test_create_master_flat():
    input = pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/FLAT/2025-06-26/RAW_Ultra")
    output = pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/FLAT/2025-06-26")
    master_bias = pathlib.Path(
        "/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/BIAS/2025-06-24/BIAS_2025-07-16_stacked.fit"
    )

    await create_calibration_master_flat(
        raw_folder=input,
        output_folder=output,
        master_bias=master_bias,
        filter_name="Ultra",
        extension=fits_extension.FITS_EXT_FIT,
    )


if __name__ == "__main__":
    asyncio.run(_test_create_master_flat())
