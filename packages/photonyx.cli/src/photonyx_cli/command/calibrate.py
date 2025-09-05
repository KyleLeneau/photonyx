from __future__ import annotations
import cappa
import structlog
import asyncio
import pathlib
import typing as t

from ..interface.app import PhotonyxApp
from ..interface.calibrate import CalibrateCommand
from ..utils.file_watcher import watch_files
from ..config.loader import find_session_config, ConfigLoaderError
from ..config.loader import find_hardware_profile


from async_siril import SirilCli
from async_siril.command import set32bits, setext, start_ls, stop_ls, livestack
from async_siril.command_types import fits_extension

from rich.prompt import Confirm


log = structlog.stdlib.get_logger()


async def invoke(app: PhotonyxApp, command: CalibrateCommand, output: cappa.Output):
    log.debug(app)
    log.debug(command)

    try:
        # Find the hardware profile by searching up the directory tree
        hardware_profile = find_hardware_profile(command.folder)
        log.debug(hardware_profile)
    except ConfigLoaderError:
        log.error("No hardware profile found")
        return

    try:
        # Find the session config file
        session_config = find_session_config(command.folder)
        log.debug(session_config)
    except ConfigLoaderError:
        log.error("No session config found")
        return

    # Calibrate each exposure in the session
    for exp in session_config.exposures:
        # Create the PP_ folder (TODO: allow deleting this)
        pp_folder = command.folder / exp.pp_folder
        if not pp_folder.exists():
            pp_folder.mkdir()
            log.debug("Created PP_ folder", folder=pp_folder)

        resolved = hardware_profile.resolve_calibration_masters(exp)
        log.debug("resolved calibration master:")
        log.debug(resolved)

        output.output("Calibration Masters found, please confirm:")
        output.output("")
        output.output(f"Raw Folder:\t{exp.raw_folder}")
        output.output(f"Master Bias:\t{resolved.bias.file if resolved.bias else "None"}")
        output.output(f"Master Dark:\t{resolved.dark.file if resolved.dark else "None"}")
        output.output(f"Master Flat:\t{resolved.flat.file if resolved.flat else "None"}")
        output.output("")
        Confirm.ask("Ready?")
