from __future__ import annotations
import asyncio
import pathlib
import shutil
import structlog.stdlib
import tempfile
import typing as t

from async_siril import ConversionFile, SirilCli
from async_siril.command import (
    setext,
    set32bits,
    convert,
    register,
    seqapplyreg,
)
from async_siril.command import fits_extension, sequence_framing


log = structlog.stdlib.get_logger()


class RegisterStackException(Exception):
    pass


async def register_linear_stack_frames(
    inputs: t.List[pathlib.Path],
    output_folder: pathlib.Path,
    extension: fits_extension = fits_extension.FITS_EXT_FITS,
):
    # Validate all the input folders exist
    validate_all_paths_exist(inputs)

    # Check if output folder exists else make it
    if not output_folder.exists():
        raise RegisterStackException("output_folder should be created ahead of time")

    log.info(
        "Starting register stack of linear_stack frames",
        inputs=inputs,
        output=output_folder,
        extension=extension,
    )

    with tempfile.TemporaryDirectory(prefix="photonyx-") as tempdir:  # type: ignore
        temp = pathlib.Path(tempdir)
        log.info(f"temp dir: {temp}")

        for input in inputs:
            shutil.copy2(input, temp)

        async with SirilCli(directory=temp) as siril:
            # Caution: these settings are saved between Siril sessions
            await siril.command(setext(extension))
            await siril.command(set32bits())

            # Manage the next prefix
            prefix = "stack_"

            # convert what's in temp directory
            await siril.command(convert(prefix, output_dir=str(temp)))

            # Register all the images
            await siril.command(register(prefix))

            # Generate their transformed version
            await siril.command(seqapplyreg(prefix, framing=sequence_framing.FRAME_MIN))
            prefix = f"r_{prefix}"

            # Move converted files
            conversion = ConversionFile(temp / "stack_conversion.txt")
            # log.info(f"conversion: {conversion.entries}")
            await _move_converted_files(output_folder, conversion, prefix="r_")

            await asyncio.sleep(1)  # 3600 to check temp
            log.info("register stacks complete")

    # TODO: Return results of this (set of paths by filter)
    log.info("Register Stack of linear stack frames completed", output_folder=output_folder)


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
        raise RegisterStackException(f"Some paths missing {non_existent_paths}")

async def _move_converted_files(output_folder: pathlib.Path, conversion: ConversionFile, prefix: str) -> None:
    log.info(f"Moving converted files to {output_folder}")
    for entry in conversion.entries:
        converted_file = conversion.file.parent.joinpath(f"{prefix}{entry.converted_file.name}")
        converted_file.rename(output_folder / f"{prefix}{entry.original_file.name}")
