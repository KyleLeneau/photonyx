# Loader & Persistence functions for configuration files

import structlog
import pathlib

from pydantic_yaml import parse_yaml_file_as, to_yaml_file
from .global_app import GlobalAppConfig, GLOBAL_CONFIG_DIR, GLOBAL_CONFIG_FILE_PATH
from .session import SessionConfig, SESSION_CONFIG_FILE_NAME
from .profile import ProfileConfig, PROFILE_CONFIG_FILE_NAME
from .project import ProjectConfig, PROJECT_CONFIG_FILE_NAME

log = structlog.get_logger()

_DEBUG = False


class ConfigLoaderError(Exception):
    """Custom exception for configuration loading errors."""

    pass


def setup_app_dir() -> None:
    """Set up the application directory structure."""
    GLOBAL_CONFIG_DIR.mkdir(parents=True, exist_ok=True)


def load_or_create_global_config() -> GlobalAppConfig:
    """Load the global application configuration or create a default one."""
    if GLOBAL_CONFIG_FILE_PATH.exists():
        if _DEBUG:
            print("Log file exists and loading from: %s", GLOBAL_CONFIG_FILE_PATH)
        return parse_yaml_file_as(GlobalAppConfig, GLOBAL_CONFIG_FILE_PATH)

    if _DEBUG:
        print(
            "No config file found, creating default config and saving to: %s",
            GLOBAL_CONFIG_FILE_PATH,
        )

    result = GlobalAppConfig()
    to_yaml_file(GLOBAL_CONFIG_FILE_PATH, result, add_comments=True)
    return result


def find_project_config(folder: pathlib.Path) -> ProjectConfig:
    config_file = folder / PROJECT_CONFIG_FILE_NAME
    if not config_file.exists():
        raise ConfigLoaderError("Project config file not found")
    config = parse_yaml_file_as(ProjectConfig, config_file)
    return config


def find_session_config(folder: pathlib.Path) -> SessionConfig:
    config_file = folder / SESSION_CONFIG_FILE_NAME
    if not config_file.exists():
        raise ConfigLoaderError("Session config file not found")
    config = parse_yaml_file_as(SessionConfig, config_file)
    config.resolve(folder)
    return config


def find_hardware_profile(folder: pathlib.Path) -> ProfileConfig:
    """Find the closest profile config file by searching up the directory tree."""
    current_path = folder.expanduser().resolve()

    while True:
        config_file = current_path / PROFILE_CONFIG_FILE_NAME
        if config_file.exists():
            config = parse_yaml_file_as(ProfileConfig, config_file)
            config.resolve()
            return config

        # TODO: add a stop at global config profile_home_directory
        parent = current_path.parent
        if parent == current_path:  # Reached filesystem root
            break
        current_path = parent

    raise ConfigLoaderError(
        f"Profile config file '{PROFILE_CONFIG_FILE_NAME}' not found in '{folder}' or any parent directory"
    )
