import cappa
import structlog

from pydantic import BaseModel

from ..config.global_app import GlobalAppConfig, LoggingConfig
from ..config.loader import setup_app_dir, load_or_create_global_config
from ..utils import logging

from .preprocess import PreprocessCommand
from .master_bias import MasterBiasCommand
from .master_dark import MasterDarkCommand
from .master_flat import MasterFlatCommand
from .stack import StackCommand
from .version import VersionCommand


log = structlog.get_logger()


def setup_logging(cfg: LoggingConfig) -> None:
    logging.setup_logging(
        force_json=None,
        logger_levels=cfg.log_levels,
        root_logger_level=logging.LogLevels.DEBUG if cfg.verbosity > 0 else logging.LogLevels.INFO,
        testing=cfg._testing,
        verbosity=cfg.verbosity,
    )


class PhotonyxApp(BaseModel):
    """Photonyx CLI application."""

    commands: cappa.Subcommands[
        VersionCommand
        # | LivestackCommand
        | PreprocessCommand
        | StackCommand
        # | ScanCommand
        | MasterBiasCommand
        | MasterDarkCommand
        | MasterFlatCommand
    ]

    _global_config: GlobalAppConfig = GlobalAppConfig()

    def model_post_init(self, context):
        # Ensure app directory exists
        setup_app_dir()

        # Load or create global config
        # TODO: Allow users to specify a new global config path
        self._global_config = load_or_create_global_config()

        # Setup logging
        setup_logging(self._global_config.logging)

        log.debug("Global configuration loaded", config=self._global_config)
        log.debug("PhotonyxApp initialized")
        return super().model_post_init(context)

    @property
    def global_config(self) -> GlobalAppConfig:
        """Get the global application configuration."""
        return self._global_config
