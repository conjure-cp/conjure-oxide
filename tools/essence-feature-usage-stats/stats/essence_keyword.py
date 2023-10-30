from stats.essence_file import EssenceFile
from utils.colour import *  # noqa: F403


class EssenceKeyword:
    """
    EssenceKeyword stores, for a particular keyword "name", the file uses of that keyword, and aggregate statistics.
    """

    # ToDo use python getters / setters instead of java style,
    #  search: "python function as attribute" or ask Nik

    # ToDo some attrs should be private?

    def __init__(self, name: str, files=None):
        if files is None:
            files = []

        self.name = name
        self.total_usages = 0
        self.min_usages = None
        self.max_usages = None

        self.file_usages = {}
        for file in files:
            self.add_file(file)

    def add_file(self, file: EssenceFile):
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
        return set(self.file_usages.keys())

    @property
    def num_files_using_feature(self) -> int:
        return len(self.files)

    @property
    def avg_usages(self) -> float:
        return float(self.total_usages) / float(
            self.num_files_using_feature,
        )

    def get_file_paths(self, depth=0) -> list:
        return [x.get_str_path(depth) for x in self.files]

    def get_usages_in_file(self, file) -> int:
        return file.get_uses(self.name)

    def as_json(self, path_depth=0) -> dict:
        return {
            "name": self.name,
            "used_in_files": self.get_file_paths(path_depth),
            "max_usages_in_file": self.max_usages,
            "min_usages_in_file": self.min_usages,
            "avg_usages_per_file": self.avg_usages,
            "total_usages": self.total_usages,
        }

    def get_colour(self, n_uses: int) -> Colour:  # noqa: F405
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
