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

async def _test_calibrate_many_sessions():
    master_dark = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/DARK/2025-11-05_DARK_master_300.0s_-10.0C.fits")
    master_flat_l = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/FLAT/2025-11-08_FLAT_master_L.fits")
    master_flat_r = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/FLAT/2025-11-08_FLAT_master_R.fits")
    master_flat_g = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/FLAT/2025-11-08_FLAT_master_G.fits")
    master_flat_b = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/FLAT/2025-11-08_FLAT_master_B.fits")
    master_flat_h = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/FLAT/2025-11-08_FLAT_master_H.fits")
    master_flat_s = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/FLAT/2025-11-08_FLAT_master_S.fits")
    master_flat_o = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/FLAT/2025-11-08_FLAT_master_O.fits")

    sessions = [
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-03/H-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-03/PP_H-1x1'),
            'dark': master_dark,
            'flat': master_flat_h
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/H-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/PP_H-1x1'),
            'dark': master_dark,
            'flat': master_flat_h
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/O-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/PP_O-1x1'),
            'dark': master_dark,
            'flat': master_flat_o
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/S-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/PP_S-1x1'),
            'dark': master_dark,
            'flat': master_flat_s
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-09/H-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-09/PP_H-1x1'),
            'dark': master_dark,
            'flat': master_flat_h
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-09/O-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-09/PP_O-1x1'),
            'dark': master_dark,
            'flat': master_flat_o
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-09/S-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-09/PP_S-1x1'),
            'dark': master_dark,
            'flat': master_flat_s
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-10/H-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-10/PP_H-1x1'),
            'dark': master_dark,
            'flat': master_flat_h
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-10/O-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-10/PP_O-1x1'),
            'dark': master_dark,
            'flat': master_flat_o
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-10/S-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-10/PP_S-1x1'),
            'dark': master_dark,
            'flat': master_flat_s
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-11/H-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-11/PP_H-1x1'),
            'dark': master_dark,
            'flat': master_flat_h
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-11/O-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-11/PP_O-1x1'),
            'dark': master_dark,
            'flat': master_flat_o
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-11/S-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-11/PP_S-1x1'),
            'dark': master_dark,
            'flat': master_flat_s
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-12/H-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-12/PP_H-1x1'),
            'dark': master_dark,
            'flat': master_flat_h
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-12/O-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-12/PP_O-1x1'),
            'dark': master_dark,
            'flat': master_flat_o
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-15/H-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-15/PP_H-1x1'),
            'dark': master_dark,
            'flat': master_flat_h
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-15/O-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-15/PP_O-1x1'),
            'dark': master_dark,
            'flat': master_flat_o
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-15/S-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-15/PP_S-1x1'),
            'dark': master_dark,
            'flat': master_flat_s
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-17/H-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-17/PP_H-1x1'),
            'dark': master_dark,
            'flat': master_flat_h
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-17/O-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-17/PP_O-1x1'),
            'dark': master_dark,
            'flat': master_flat_o
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-17/S-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-17/PP_S-1x1'),
            'dark': master_dark,
            'flat': master_flat_s
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-18/H-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-18/PP_H-1x1'),
            'dark': master_dark,
            'flat': master_flat_h
        },
        {
            'raw': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-18/O-1x1'),
            'output': pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-18/PP_O-1x1'),
            'dark': master_dark,
            'flat': master_flat_o
        },
    ]

    for s in sessions:
        if s['output'].exists():
            continue

        s['output'].mkdir()
        await calibrate_raw_light_frames(
            raw_folder=s['raw'],
            output_folder=s['output'],
            master_bias=None,
            master_dark=s['dark'],
            master_flat=s['flat'],
            extension=fits_extension.FITS_EXT_FITS,
        )


if __name__ == "__main__":
    asyncio.run(_test_calibrate_many_sessions())
