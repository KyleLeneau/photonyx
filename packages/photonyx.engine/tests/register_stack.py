import asyncio
import pathlib
from photonyx_engine.register_stack import register_linear_stack_frames


async def _test_register_linear_stack():
    inputs = [
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/PROJECTS/LDN 1272/PP/2025-11-08_linear_stack_H.fits'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/PROJECTS/LDN 1272/PP/2025-11-08_linear_stack_O.fits'),
        pathlib.Path('/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/PROJECTS/LDN 1272/PP/2025-11-08_linear_stack_S.fits'),
    ]
    output = pathlib.Path("/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/PROJECTS/LDN 1272/PP/")
    # output.mkdir()

    await register_linear_stack_frames(inputs=inputs, output_folder=output)


if __name__ == "__main__":
    asyncio.run(_test_register_linear_stack())
