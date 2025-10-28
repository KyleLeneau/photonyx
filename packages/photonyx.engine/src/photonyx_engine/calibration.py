from __future__ import annotations
import asyncio
import pathlib
import structlog.stdlib
import tempfile
import typing as t

from photonyx_engine.utils import all_color_raw_frames
from async_siril import SirilCli, ConversionFile
from async_siril.command import setext, set32bits, cd, convert, calibrate
from async_siril.command import fits_extension


log = structlog.stdlib.get_logger()


class CalibrationException(Exception):
    pass


async def calibrate_raw_light_frames(
    raw_folder: pathlib.Path,
    output_folder: pathlib.Path,
    master_bias: t.Optional[pathlib.Path] = None,
    master_dark: t.Optional[pathlib.Path] = None,
    master_flat: t.Optional[pathlib.Path] = None,
    extension: fits_extension = fits_extension.FITS_EXT_FITS,
) -> pathlib.Path:
    # Check if input folder exists
    if not raw_folder.exists():
        raise CalibrationException("raw_folder is missing")

    # Check if output folder exists else make it
    if not output_folder.exists():
        raise CalibrationException("output_folder should be created ahead of time")

    if master_bias is None and master_dark is None and master_flat is None:
        log.error("Dark, flat, or bias not specified")
        raise CalibrationException("one of the master calibration should be passed")

    # Check if all raw frames are OSC
    all_color = all_color_raw_frames(raw_folder, extension)
    log.info(f"Raw images are OSC: {all_color}")

    log.info(
        "Starting calibration light frames",
        raw_folder=raw_folder,
        output=output_folder,
        master_bias=master_bias,
        master_dark=master_dark,
        master_flat=master_flat,
        extension=extension,
        all_color=all_color,
    )

    with tempfile.TemporaryDirectory(prefix="photonyx-") as tempdir:  # type: ignore
        temp = pathlib.Path(tempdir)
        log.info(f"temp dir: {temp}")

        async with SirilCli(directory=tempdir) as siril:
            # Caution: these settings are saved between Siril sessions
            await siril.command(setext(extension))
            await siril.command(set32bits())

            # Move to the raw folder to convert into a sequence
            await siril.command(cd(str(raw_folder.resolve())))
            await siril.command(convert("light_", output_dir=str(temp)))

            # Return to working directory
            await siril.command(cd(str(temp)))

            # Run calibration
            await siril.command(
                calibrate(
                    "light_",
                    bias=f"{str(master_bias)}" if master_bias is not None else None,
                    dark=f"{str(master_dark)}" if master_dark is not None else None,
                    flat=f"{str(master_flat)}" if master_flat is not None else None,
                    cfa=all_color,
                    debayer=all_color,
                    equalize_cfa=all_color,
                )
            )

            conversion = ConversionFile(temp / "light_conversion.txt")
            # log.info(f"conversion: {conversion.entries}")
            await _move_converted_files(output_folder, conversion, prefix="pp_")
            await asyncio.sleep(1)
            log.info("calibration ended")

    log.info("Calibration of light frames completed", output_folder=output_folder)
    return output_folder


async def _move_converted_files(output_folder: pathlib.Path, conversion: ConversionFile, prefix: str) -> None:
    log.info(f"Moving converted files to {output_folder}")
    for entry in conversion.entries:
        converted_file = conversion.file.parent.joinpath(f"{prefix}{entry.converted_file.name}")
        converted_file.rename(output_folder / f"{prefix}{entry.original_file.name}")
