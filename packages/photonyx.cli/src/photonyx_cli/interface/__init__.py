import typing as t
import pathlib
import structlog

from ..config.loader import ConfigLoaderError, find_hardware_profile
from ..config.profile import ProfileConfig

log = structlog.stdlib.get_logger()

# Type alias for profile path attributes
ProfilePathAttr = t.Literal["bias", "dark", "flat", "light", "projects"]


class BaseProfileCommand:
    _hardware_profile: t.Optional[ProfileConfig] = None

    def load_profile(self, path: pathlib.Path):
        try:
            # Find the hardware profile by searching up the directory tree
            self._hardware_profile = find_hardware_profile(path)
            log.debug("found hardware profile", profile=self._hardware_profile.home)
        except ConfigLoaderError:
            log.warn("No hardware profile found")
            self._hardware_profile = None

    def resolve_output(
        self, output: t.Optional[pathlib.Path], profile_attr: ProfilePathAttr
    ) -> t.Optional[pathlib.Path]:
        """Resolve output path using profile default if not provided.

        Args:
            output: The explicitly provided output path (or None)
            profile_attr: The profile attribute to use as fallback (e.g., 'bias', 'dark', 'flat')

        Returns:
            The resolved output path, or None if no path could be determined
        """
        if output is None and self.profile is not None:
            profile_path = getattr(self.profile, profile_attr)
            if profile_path.exists():
                return profile_path
        return output

    @property
    def profile(self) -> ProfileConfig | None:
        return self._hardware_profile
