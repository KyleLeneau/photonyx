from __future__ import annotations
import cappa
import structlog

from rich.prompt import Confirm

from ..interface.app import PhotonyxApp
from ..interface.calibrate import CalibrateCommand
from ..config.loader import find_session_config, ConfigLoaderError
from ..config.loader import find_hardware_profile
from photonyx_engine.calibration import calibrate_raw_light_frames, CalibrationException

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
        if not exp.pp_folder.exists():
            exp.pp_folder.mkdir()
            log.debug("Created PP_ folder", folder=exp.pp_folder)
        else:
            log.warn("Skipping exposure since it exists already", exposure=exp)
            continue

        resolved = hardware_profile.resolve_calibration_masters(exp)
        log.debug("resolved calibration master:")
        log.debug(resolved)

        output.output("Calibration Masters found, please confirm:")
        output.output("")
        output.output(f"Raw Folder:\t{exp.raw_folder}")
        output.output(f"Master Bias:\t{resolved.bias.file if resolved.bias else 'None'}")
        output.output(f"Master Dark:\t{resolved.dark.file if resolved.dark else 'None'}")
        output.output(f"Master Flat:\t{resolved.flat.file if resolved.flat else 'None'}")
        output.output("")

        if Confirm.ask("Ready?"):
            try:
                await calibrate_raw_light_frames(
                    raw_folder=exp.raw_folder,
                    output_folder=exp.pp_folder,
                    master_bias=resolved.bias.file if resolved.bias else None,
                    master_dark=resolved.dark.file if resolved.dark else None,
                    master_flat=resolved.flat.file if resolved.flat else None,
                )
                output.output("Done")
            except CalibrationException as e:
                log.error(e)
