from __future__ import annotations
import asyncio
import datetime
import pathlib
import structlog.stdlib
import tempfile
import typing as t

from photonyx_engine.utils import is_color_frame
from async_siril import SirilCli
from async_siril.helpers import BestRejection
from async_siril.command import (
    setext,
    set32bits,
    cd,
    convert,
    seqsubsky,
    register,
    seqapplyreg,
    stack,
    load,
    mirrorx,
    save,
    split,
)
from async_siril.command import fits_extension, stack_norm


log = structlog.stdlib.get_logger()


class LinearStackException(Exception):
    pass


async def linear_stack_light_frames(
    input_folders: t.List[pathlib.Path],
    output_folder: pathlib.Path,
    filter_name: str,
    suffix: t.Optional[str] = None,
    extract_background: bool = False,
    split_color: bool = False,
    extension: fits_extension = fits_extension.FITS_EXT_FITS,
):
    # Validate all the input folders exist
    validate_all_paths_exist(input_folders)

    # Check if output folder exists else make it
    if not output_folder.exists():
        raise LinearStackException("output_folder should be created ahead of time")

    log.info(
        "Starting linear stack of light frames",
        inputs=input_folders,
        output=output_folder,
        filter_name=filter_name,
        suffix=suffix,
        extract_background=extract_background,
        split_color=split_color,
        extension=extension,
    )

    with tempfile.TemporaryDirectory(prefix="photonyx-") as tempdir:  # type: ignore
        temp = pathlib.Path(tempdir)
        log.info(f"temp dir: {temp}")

        async with SirilCli(directory=tempdir) as siril:
            # Caution: these settings are saved between Siril sessions
            await siril.command(setext(extension))
            await siril.command(set32bits())

            # Manage the next prefix
            prefix = "light_"

            # convert each input directory
            start_idx = 1
            for input in input_folders:
                await siril.command(cd(str(input.resolve())))
                await siril.command(convert(prefix, output_dir=str(temp), start_index=start_idx))
                start_idx += len(list(input.glob(f"*.{extension.value}")))

            # Return to working directory
            await siril.command(cd(str(temp)))

            # Optional: run bg extraction on every frame before stacking
            if extract_background:
                await siril.command(seqsubsky(prefix))
                prefix = f"bkg_{prefix}"

            # Register all the images
            await siril.command(register(prefix, two_pass=True))

            # Generate their transformed version
            await siril.command(seqapplyreg(prefix))
            prefix = f"r_{prefix}"

            # Find the best rejection method
            rejection = BestRejection.find(list(temp.glob(f"*.{extension.value}")))
            log.info("Found best stacking rejection", rejection=rejection)

            # Stack the background extracted images
            await siril.command(
                stack(
                    prefix,
                    norm=stack_norm.NORM_ADD_SCALE,
                    filter_included=True,
                    output_norm=True,
                    rgb_equalization=True,
                    rejection=rejection.method,
                    lower_rej=rejection.low_threshold,
                    higher_rej=rejection.high_threshold,
                    out="result",
                )
            )

            # Load and flip the image if needed
            await siril.command(load("result"))
            await siril.command(mirrorx())
            await siril.command(save("result"))

            # Save to final output location
            current_datetime = datetime.datetime.now()
            date = current_datetime.strftime("%Y-%m-%d")

            file_suffix = f"_{suffix}" if suffix is not None else ""
            filter_output_file = output_folder / f"{date}_linear_stack_{filter_name}{file_suffix}"
            await siril.command(save(str(filter_output_file)))

            # Split and save RGB from OSC image
            if split_color:
                osc_result = is_color_frame(pathlib.Path(f"{filter_output_file}.{extension.value}"))
                if osc_result:
                    log.info("Splitting color image into RGB layers")
                    _r = output_folder / f"{date}_linear_stack_R{file_suffix}"
                    _g = output_folder / f"{date}_linear_stack_G{file_suffix}"
                    _b = output_folder / f"{date}_linear_stack_B{file_suffix}"
                    await siril.command(split(_r, _g, _b))
                else:
                    log.warn("Can not split non-color result")

            await asyncio.sleep(1)  # 3600 to check temp
            log.info("linear stacking complete")

    # TODO: Return results of this (set of paths by filter)
    log.info("Linear Stack of light frames completed", output_folder=output_folder)


def validate_all_paths_exist(path_list: t.List[pathlib.Path]):
    """
    Checks all the folders exist otherwise raises an exception

    Args:
        path_list (list): A list of pathlib.Path objects.
    """
    non_existent_paths = []
    for path_obj in path_list:
        if not path_obj.exists():
            non_existent_paths.append(path_obj)

    if len(non_existent_paths) > 0:
        raise LinearStackException(f"Some paths missing {non_existent_paths}")
