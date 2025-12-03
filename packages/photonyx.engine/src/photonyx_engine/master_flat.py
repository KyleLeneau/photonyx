from __future__ import annotations
import asyncio
import pathlib
import structlog.stdlib
import tempfile
import typing as t

from async_siril import SirilCli
from async_siril.command import setext, set32bits, cd, convert, stack, calibrate
from async_siril.command import fits_extension, stack_norm
from photonyx_engine.utils import first_observation_date_filter, all_fits_files


log = structlog.stdlib.get_logger()


class CalibrationMasterFlatException(Exception):
    pass


async def create_calibration_master_flat(
    raw_folder: pathlib.Path,
    output_folder: pathlib.Path,
    master_bias: pathlib.Path,
    filter_name: t.Optional[str] = None,
    extension: fits_extension = fits_extension.FITS_EXT_FITS,
) -> pathlib.Path:
    # Check if input folder exists
    if not raw_folder.exists():
        raise CalibrationMasterFlatException("raw_folder is missing")

    # Check to make sure the folder has files to work with
    raw_files = all_fits_files(raw_folder)
    if not raw_files or len(raw_files) == 0:
        raise CalibrationMasterFlatException("raw_folder contains no files")

    # Check if output folder exists else make it
    if not output_folder.exists():
        raise CalibrationMasterFlatException("output_folder should be created ahead of time")

    # Check if output folder exists else make it
    if not master_bias.exists():
        raise CalibrationMasterFlatException("master_bias can not be found")

    log.info(
        "Starting create calibration master flat frame",
        raw_folder=raw_folder,
        output=output_folder,
        master_bias=master_bias,
        filter_name=filter_name,
        extension=extension,
    )

    # Setup output
    obs_datetime, obs_filter = first_observation_date_filter(raw_folder)
    used_filter = obs_filter if len(obs_filter) > 0 else filter_name
    if used_filter is None:
        raise CalibrationMasterFlatException("unknown filter_name for sequence")

    date = obs_datetime.strftime("%Y-%m-%d")
    output_file = output_folder / f"{date}_FLAT_master_{used_filter}"

    with tempfile.TemporaryDirectory(prefix="photonyx-") as tempdir:  # type: ignore
        temp = pathlib.Path(tempdir)
        log.info(f"temp dir: {temp}")

        async with SirilCli(directory=tempdir) as siril:
            # Caution: these settings are saved between Siril sessions
            await siril.command(setext(extension))
            await siril.command(set32bits())

            # Move to the raw folder to convert into a sequence
            await siril.command(cd(str(raw_folder.resolve())))
            await siril.command(convert("flat_", output_dir=str(temp)))

            # Return to working directory
            await siril.command(cd(str(temp)))

            # Calibrate the flat frames using the master bias
            await siril.command(calibrate("flat_", bias=str(master_bias)))

            # Stack with defaults
            await siril.command(stack("pp_flat_", out=str(output_file), norm=stack_norm.NORM_MUL))

            await asyncio.sleep(1)
            log.info("master creation ended")

    result = pathlib.Path(str(output_file) + f".{extension.value}")
    if not result.exists():
        raise CalibrationMasterFlatException(f"Output file missing: {result}")

    log.info("Master FLAT stacking completed", output_folder=output_folder, result=result)
    return result
