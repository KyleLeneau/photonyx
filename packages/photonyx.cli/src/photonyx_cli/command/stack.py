from __future__ import annotations
import cappa
import structlog

from ..interface.app import PhotonyxApp
from ..interface.stack import StackCommand
from ..config.loader import find_session_config, find_project_config, ConfigLoaderError
from ..config.loader import find_hardware_profile

from rich.prompt import Confirm


log = structlog.stdlib.get_logger()


async def invoke(app: PhotonyxApp, command: StackCommand, output: cappa.Output):
    log.info(app)
    log.info(command)

    try:
        # Find the hardware profile by searching up the directory tree
        hardware_profile = find_hardware_profile(command.folder)
        log.debug(hardware_profile)
    except ConfigLoaderError:
        log.error("No hardware profile found")
        return

    try:
        # Find the project config file
        project_config = find_project_config(command.folder)
        project_config.validate(hardware_profile.profile_home)
        log.debug(project_config)
    except ConfigLoaderError:
        log.error("No project config found")
        return

    # Resolve all session configs
    resolved_sessions = []
    for s in project_config.sessions:
        print(s.session_folder)
        session_config = find_session_config(hardware_profile.profile_home / s.session_folder)
        resolved_sessions.append(session_config)
        log.debug(session_config)

    Confirm.ask("Continue")
