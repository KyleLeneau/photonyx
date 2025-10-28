import asyncio
import pathlib
from photonyx_engine.linear_stack import linear_stack_light_frames


async def _test_linear_stack_mono():
    inputs = [
        pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-03/H-1x1/"),
        pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/H-1x1/"),
    ]
    output = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/PROJECTS/LDN 1272/test/")
    filter_name = "H"

    await linear_stack_light_frames(
        input_folders=inputs, output_folder=output, filter_name=filter_name, extract_background=True
    )


async def _test_linear_stack_osc():
    inputs = [
        pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/LIGHT/NGC_6992_Veil/2025-06-26/PP_300_Ultra/"),
    ]
    output = pathlib.Path("/Users/kyle/Pictures/Astro/Radian_75_71mc_pro/PROJECTS/6992_Veil")
    filter_name = "Ultra"

    await linear_stack_light_frames(
        input_folders=inputs, output_folder=output, filter_name=filter_name, extract_background=True, split_color=True
    )


if __name__ == "__main__":
    # asyncio.run(_test_linear_stack_mono())
    asyncio.run(_test_linear_stack_osc())
