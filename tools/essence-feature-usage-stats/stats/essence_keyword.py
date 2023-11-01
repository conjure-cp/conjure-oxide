from typing import Dict

from stats.essence_file import EssenceFile
from utils.colour import *  # noqa: F403


class EssenceKeyword:
    """EssenceKeyword stores, for a particular keyword "name", the file uses of that keyword, and aggregate statistics."""

    def __init__(self, name: str, files=None):
        """
        Create a new EssenceKeyword object.

        :param name: The Essence keyword
        :param files: Collection of files that use it (more can be added after creation)
        """
        if files is None:
            files = []

        self.name = name
        self.total_usages = 0
        self.min_usages = None
        self.max_usages = None

        self._file_usages = {}
        for file in files:
            self.add_file(file)

    @property
    def file_usages(self) -> Dict[EssenceFile, int]:
        """Get a dictionary of EssenceFile objects and usages of this keyword in these files."""
        return self._file_usages

    def add_file(self, file: EssenceFile):
        """Add a file that uses this EssenceKeyword to the stats."""
        if file not in self.file_usages and file.get_uses(self.name) > 0:
            usages = file.get_uses(self.name)
            self.file_usages[file] = usages
            self.total_usages += usages

            if self.max_usages is None:
                self.max_usages = usages
            else:
                self.max_usages = max(self.max_usages, usages)

            if self.min_usages is None:
                self.min_usages = usages
            else:
                self.min_usages = min(self.min_usages, usages)

    @property
    def files(self):
        """Get all files that use this keyword."""
        return set(self.file_usages.keys())

    @property
    def num_files_using_keyword(self) -> int:
        """Get number of files that use this Essence keyword."""
        return len(self.files)

    @property
    def avg_usages(self) -> float:
        """Get the average number of usages of this keyword per file."""
        return float(self.total_usages) / float(
            self.num_files_using_keyword,
        )

    def get_file_paths(self, depth=0) -> list:
        """
        Get paths to files that use this essence keyword, trimmed to a given depth.

        :param depth: trim file paths, leaving a part of the path of this length (from the end).
        """
        return [x.get_str_path(depth) for x in self.files]

    def get_usages_in_file(self, file) -> int:
        """Get how often this Essence keyword is used in the given file."""
        return file.get_uses(self.name)

    def as_json(self, path_depth=0) -> dict:
        """Get data for this Essence keyword as a JSON."""
        return {
            "name": self.name,
            "used_in_files": self.get_file_paths(path_depth),
            "max_usages_in_file": self.max_usages,
            "min_usages_in_file": self.min_usages,
            "avg_usages_per_file": self.avg_usages,
            "total_usages": self.total_usages,
        }

    def get_colour(self, n_uses: int) -> Colour:  # noqa: F405
        """Get colour to use for this keyword's corresponding table cell."""
        avg = int(self.avg_usages)

        if n_uses == 0:
            return RED  # noqa: F405
        if n_uses < avg:
            return get_linear_gradient_value(  # noqa: F405
                n_uses,
                self.min_usages,
                avg,
                HOT_ORANGE,  # noqa: F405
                YELLOW,  # noqa: F405
            )

        return get_linear_gradient_value(  # noqa: F405
            n_uses,
            avg,
            self.max_usages,
            YELLOW,  # noqa: F405
            GREEN,  # noqa: F405
        )
