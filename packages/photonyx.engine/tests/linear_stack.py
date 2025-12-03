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


async def _test_linear_stack_multiple():
    inputs_h = [
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-03/PP_H-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/PP_H-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-09/PP_H-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-10/PP_H-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-11/PP_H-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-12/PP_H-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-15/PP_H-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-17/PP_H-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-18/PP_H-1x1'),
    ]
    inputs_o = [
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/PP_O-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-09/PP_O-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-10/PP_O-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-11/PP_O-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-12/PP_O-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-15/PP_O-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-17/PP_O-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-18/PP_O-1x1'),
    ]
    inputs_s = [
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-06/PP_S-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-09/PP_S-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-10/PP_S-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-11/PP_S-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-15/PP_S-1x1'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/LDN 1272/2025-10-17/PP_S-1x1'),
    ]
    output = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/PROJECTS/LDN 1272/PP/")
    output.mkdir()

    await linear_stack_light_frames(
        input_folders=inputs_h, output_folder=output, filter_name="H", extract_background=True
    )

    await linear_stack_light_frames(
        input_folders=inputs_o, output_folder=output, filter_name="O", extract_background=True
    )

    await linear_stack_light_frames(
        input_folders=inputs_s, output_folder=output, filter_name="S", extract_background=True
    )

async def _test_linear_stack_4p():
    output = pathlib.Path("/Users/kyle/Pictures/Astro/WO91_6200mc_pro/PROJECTS/NGC7000_mosiac_4p/v4")
    p_1_1 = pathlib.Path("/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/2025-06-25_NGC_7000_4P/PP_L-Ultimate_1-1")
    p_1_2 = pathlib.Path("/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/2025-06-25_NGC_7000_4P/PP_L-Ultimate_1-2")
    p_2_1 = pathlib.Path("/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/2025-06-25_NGC_7000_4P/PP_L-Ultimate_2-1")
    p_2_2 = pathlib.Path("/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/2025-06-25_NGC_7000_4P/PP_L-Ultimate_2-2")
    p_center = pathlib.Path("/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/2025-06-25_NGC_7000_4P/PP_L-Ultimate_center")
    await linear_stack_light_frames(
        input_folders=[p_1_1], output_folder=output, filter_name="L-Ultimate", suffix="1-1", extract_background=True
    )
    await linear_stack_light_frames(
        input_folders=[p_1_2], output_folder=output, filter_name="L-Ultimate", suffix="1-2", extract_background=True
    )
    await linear_stack_light_frames(
        input_folders=[p_2_1], output_folder=output, filter_name="L-Ultimate", suffix="2-1", extract_background=True
    )
    await linear_stack_light_frames(
        input_folders=[p_2_2], output_folder=output, filter_name="L-Ultimate", suffix="2-2", extract_background=True
    )
    await linear_stack_light_frames(
        input_folders=[p_center], output_folder=output, filter_name="L-Ultimate", suffix="center", extract_background=True
    )

async def _test_linear_stack_andromeda():
    output = pathlib.Path("/Users/kyle/Pictures/Astro/WO91_6200mc_pro/PROJECTS/2025-11-13_Andromeda/v1")
    input_l = [
        pathlib.Path('/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/Andromeda Nebula/2025-10-20/PP_L'),
        pathlib.Path('/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/Andromeda Nebula/2025-10-21/PP_L'),
        pathlib.Path('/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/Andromeda Nebula/2025-10-22/PP_L'),
        pathlib.Path('/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/Andromeda Nebula/2025-10-25/PP_L'),
        pathlib.Path('/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/Andromeda Nebula/2025-10-26/PP_L'),
    ]
    input_lu = [
        pathlib.Path('/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/Andromeda Nebula/2025-10-26/PP_L-Ultimate'),
        pathlib.Path('/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/Andromeda Nebula/2025-10-27/PP_L-Ultimate'),
        pathlib.Path('/Users/kyle/Pictures/Astro/WO91_6200mc_pro/LIGHT/Andromeda Nebula/2025-10-28/PP_L-Ultimate'),
    ]
    # await linear_stack_light_frames(
    #     input_folders=input_l, output_folder=output, filter_name="L", extract_background=True
    # )
    await linear_stack_light_frames(
        input_folders=input_lu, output_folder=output, filter_name="L-Ultimate", extract_background=True
    )
    

if __name__ == "__main__":
    # asyncio.run(_test_linear_stack_mono())
    # asyncio.run(_test_linear_stack_osc())
    # asyncio.run(_test_linear_stack_multiple())
    # asyncio.run(_test_linear_stack_4p())
    asyncio.run(_test_linear_stack_andromeda())
