# Loader & Persistence functions for configuration files

import structlog

from pydantic_yaml import parse_yaml_file_as, to_yaml_file
from .global_app import GlobalAppConfig, GLOBAL_CONFIG_DIR, GLOBAL_CONFIG_FILE_PATH

log = structlog.get_logger()

_DEBUG = False


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
