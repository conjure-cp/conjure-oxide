import os
from pathlib import Path
from typing import Generator

from utils.conjure import get_essence_file_ast
from utils.files import count_lines, trim_path
from utils.misc import flat_keys_count


class EssenceFileError(ValueError):
    pass


class EssenceFileInvalidPathError(EssenceFileError):
    def __init__(self, fpath):
        super().__init__(f"Not a valid Essence file: {fpath}")


class EssenceFileNotParsableError(EssenceFileError):
    def __init__(self, fpath, msg=None):
        message = f"Essence file could not be parsed: {fpath}"
        if msg:
            message += f", reason: {msg}"

        super().__init__(message)


class EssenceInvalidDirectoryError(ValueError):
    def __init__(self, dir_path):
        super().__init__(f"The provided path '{dir_path}' is not a valid directory")


def find_essence_files(dir_path: str | Path):
    """
    Find all essence files in a given directory and return a list of full paths to them
    :param dir_path: path to directory
    :return: a generator of paths to essence files
    """

    dir_path = Path(dir_path)

    # Ensure the directory path is valid
    if not dir_path.is_dir():
        raise EssenceInvalidDirectoryError

    # Walk through the directory and its subdirectories
    for root, _, files in os.walk(dir_path):
        for file in files:
            fpath = Path(root) / file
            if fpath.is_file() and fpath.suffix == ".essence":
                yield fpath


class EssenceFile:
    """
    EssenceFile stores keyword counts and number of lines for a given file "fpath".
    """

    def __init__(self, fpath: str | Path, conjure_bin_path, blocklist=None):
        """
        Constructs a EssenceFile object from a given file path
        """

        fpath = Path(fpath).resolve()

        if not (fpath.is_file() and fpath.suffix == ".essence"):
            raise EssenceFileInvalidPathError(fpath)
        try:
            self._fpath = Path.resolve(fpath)
            self._ast = get_essence_file_ast(
                self._fpath,
                conjure_bin_path=conjure_bin_path,
            )
            self._keyword_counts = flat_keys_count(self._ast, blocklist)
            self._n_lines = count_lines(fpath)
        except Exception as e:
            raise EssenceFileNotParsableError(fpath, str(e)) from e

    @property
    def path(self) -> Path:
        return self._fpath

    @property
    def ast(self) -> dict:
        return self._ast

    @property
    def keyword_counts(self) -> dict[str, int]:
        return self._keyword_counts

    @property
    def keywords(self) -> set:
        return set(self._keyword_counts.keys())

    @property
    def n_lines(self) -> int:
        return self._n_lines

    def get_str_path(self, depth=0) -> str:
        """
        Get a formatted path to this essence file (and optionally trim it)
        :param depth: (optional) trim path, leaving a suffix of this size
        :return: formatted path to file
        """
        return trim_path(self._fpath, depth)

    def get_uses(self, keyword: str) -> int:
        """
        Get the number of times a given keyword is used in the file
        :param keyword: (str) the Essence keyword to count
        :return: how many times this keyword is used in the file
        """
        return self._keyword_counts.get(keyword, 0)

    def __hash__(self):
        return hash(self._fpath)

    def __eq__(self, other):
        return self._fpath == other._fpath

    def __str__(self):
        return f"EssenceFile({self._fpath}): {self.n_lines} lines"

    def as_json(self, path_depth=0) -> dict:
        """
        Get file stats in json format
        :param path_depth: (optional) trim path, leaving a suffix of this size
        :return: (dict) file stats, including its path, number of lines, keywords and AST
        """
        return {
            "path": self.get_str_path(path_depth),
            "ast": self._ast,
            "keyword_counts": self._keyword_counts,
            "n_lines": self.n_lines,
        }

    @staticmethod
    def get_essence_files_from_dir(
        dir_path: str | Path, conjure_bin_path: str | Path, blocklist=None
    ):
        """
        :param dir_path: path to directory with essence files
        :param conjure_bin_path: a path to conjure binary
        :param blocklist: a list of Essence keywords to ignore
        """
        for fpath in find_essence_files(dir_path):
            try:
                file = EssenceFile(fpath, conjure_bin_path, blocklist=blocklist)
                yield file
            except Exception as e:  # noqa: PERF203
                print(f'Could not process file "{fpath}", throws exception: {e}')
