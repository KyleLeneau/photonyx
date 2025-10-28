from __future__ import annotations
import asyncio
import datetime
import pathlib
import structlog.stdlib
import tempfile
import typing as t

from async_siril import SirilCli
from async_siril.command import setext, set32bits, cd, convert, stack
from async_siril.command import fits_extension


log = structlog.stdlib.get_logger()


class CalibrationMasterBiasException(Exception):
    pass


async def create_calibration_master_bias(
    raw_folder: pathlib.Path,
    output_folder: pathlib.Path,
    name_suffix: t.Optional[str] = None,
    extension: fits_extension = fits_extension.FITS_EXT_FITS,
) -> pathlib.Path:
    # Check if input folder exists
    if not raw_folder.exists():
        raise CalibrationMasterBiasException("raw_folder is missing")

    # Check if output folder exists else make it
    if not output_folder.exists():
        raise CalibrationMasterBiasException("output_folder should be created ahead of time")

    log.info(
        "Starting create calibration master bias frame",
        raw_folder=raw_folder,
        output=output_folder,
        name_suffix=name_suffix,
        extension=extension,
    )

    # Setup output
    current_datetime = datetime.datetime.now()
    date = current_datetime.strftime("%Y-%m-%d")
    output_file = output_folder / f"{date}_BIAS_master{name_suffix.ljust(1, '_') if name_suffix else ''}"

    with tempfile.TemporaryDirectory(prefix="photonyx-") as tempdir:  # type: ignore
        temp = pathlib.Path(tempdir)
        log.info(f"temp dir: {temp}")

        async with SirilCli(directory=tempdir) as siril:
            # Caution: these settings are saved between Siril sessions
            await siril.command(setext(extension))
            await siril.command(set32bits())

            # Move to the raw folder to convert into a sequence
            await siril.command(cd(str(raw_folder.resolve())))
            await siril.command(convert("bias_", output_dir=str(temp)))

            # Return to working directory
            await siril.command(cd(str(temp)))

            # Stack with defaults
            await siril.command(stack("bias_", out=str(output_file)))

            await asyncio.sleep(1)
            log.info("master creation ended")

    result = pathlib.Path(str(output_file) + f".{extension.value}")
    if not result.exists():
        raise CalibrationMasterBiasException(f"Output file missing: {result}")

    log.info("Master BIAS stacking completed", output_folder=output_folder, result=result)
    return result
