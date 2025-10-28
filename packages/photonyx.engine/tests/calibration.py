import asyncio
import pathlib
from photonyx_engine.calibration import calibrate_raw_light_frames
from async_siril.command_types import fits_extension


async def _test_calibrate_single_session():
    input = pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/LIGHT/NGC_6992_Veil/2025-06-26/RAW_300_Ultra/")
    output = pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/LIGHT/NGC_6992_Veil/2025-06-26/PP_300_Ultra")
    master_bias = None  # pathlib.Path("")
    master_dark = pathlib.Path(
        "/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/DARK/2025-06-24/DARK_2025-06-24_300s_-10C_stacked.fit"
    )
    master_flat = pathlib.Path(
        "/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/FLAT/2025-06-26/pp_FLAT_Ultra_2025-06-24_stacked.fit"
    )

    await calibrate_raw_light_frames(
        raw_folder=input,
        output_folder=output,
        master_bias=master_bias,
        master_dark=master_dark,
        master_flat=master_flat,
        extension=fits_extension.FITS_EXT_FIT,
    )


if __name__ == "__main__":
    asyncio.run(_test_calibrate_single_session())
