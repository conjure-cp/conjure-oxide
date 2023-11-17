import os
import re
from pathlib import Path
from typing import Iterable, Optional

from git import Repo
from tqdm import tqdm

from utils.conjure import get_essence_file_ast
from utils.files import count_lines, trim_path
from utils.misc import flat_keys_count


class EssenceFileError(ValueError):
    """Parent class for all errors related to parsing Essence files."""


class EssenceFileInvalidPathError(EssenceFileError):
    """Thrown when a path to an Essence file is invalid."""

    def __init__(self, fpath):  # noqa: D107
        super().__init__(f"Not a valid Essence file: {fpath}")


class EssenceFileNotParsableError(EssenceFileError):
    """Thrown when an Essence file cannot be parsed."""

    def __init__(self, fpath, msg=None):  # noqa: D107
        message = f"Essence file could not be parsed: {fpath}"
        if msg:
            message += f", reason: {msg}"

        super().__init__(message)


class EssenceInvalidDirectoryError(ValueError):
    """Thrown when a given directory with Essence files is not a valid directory."""

    def __init__(self, dir_path):  # noqa: D107
        super().__init__(f"The provided path '{dir_path}' is not a valid directory")


def find_essence_files(dir_path: str | Path, exclude_regex: str | None = None):
    """
    Find all essence files in a given directory and return a list of full paths to them.

    :param dir_path: path to directory
    :return: a generator of paths to essence files.
    :param exclude_regex: regular expression to exclude certain paths.
    """
    dir_path = Path(dir_path)

    # Ensure the directory path is valid
    if not dir_path.is_dir():
        raise EssenceInvalidDirectoryError

    if exclude_regex is None:
        exclude_regex = r"^$"  # If not excluding anything, set exclude regex to just match an empty string
    pattern = re.compile(exclude_regex)

    # Walk through the directory and its subdirectories
    for root, _, files in os.walk(dir_path):
        for file in files:
            fpath = Path(root) / file
            if (
                fpath.is_file()
                and fpath.suffix == ".essence"
                and not pattern.match(str(fpath))
            ):
                yield fpath


class EssenceFile:
    """EssenceFile stores keyword counts and number of lines for a given file "fpath"."""

    def __init__(self, fpath: str | Path, conjure_bin_path, repo=None, blocklist=None):
        """Construct an EssenceFile object from a given file path."""
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
            self._repo = repo
        except Exception as e:
            raise EssenceFileNotParsableError(fpath, str(e)) from e

    @property
    def repo(self) -> Repo | None:
        """Get the git repo that this file belongs to."""
        return self._repo

    def get_repo_name(self, depth=0) -> str | None:
        """Get the repo name, trimmed to a given depth."""
        if isinstance(self.repo, Repo):
            return trim_path(self.repo.working_dir, depth)
        return None

    @property
    def path(self) -> Path:
        """Get path to this file."""
        return self._fpath

    @property
    def ast(self) -> dict:
        """Get the AST of this file, as provided by the `conjure pretty` tool."""
        return self._ast

    @property
    def keyword_counts(self) -> dict[str, int]:
        """Get a dictionary of Essence keywords and how often they appear in this file."""
        return self._keyword_counts

    @property
    def keywords(self) -> set[str]:
        """Get a set of Essence keywords used in the file."""
        return set(self._keyword_counts.keys())

    @property
    def n_lines(self) -> int:
        """Get number of lines in the file."""
        return self._n_lines

    def get_str_path(self, depth=0) -> str:
        """
        Get a formatted path to this essence file (and optionally trim it).

        :param depth: (optional) trim path, leaving a suffix of this size
        :return: formatted path to file.
        """
        return trim_path(self._fpath, depth)

    def get_uses(self, keyword: str) -> int:
        """
        Get the number of times a given keyword is used in the file.

        :param keyword: (str) the Essence keyword to count
        :return: how many times this keyword is used in the file.
        """
        return self._keyword_counts.get(keyword, 0)

    def __hash__(self):
        """Compute a hash of this EssenceFile object. The hash of the file's path is used."""
        return hash(self._fpath)

    def __eq__(self, other):
        """EssenceFile objects are considered equal if their paths are the same."""
        return self.path == other.path

    def __str__(self):  # noqa: D105
        return f"EssenceFile({self._fpath}): {self.n_lines} lines"

    def as_json(self, path_depth=0) -> dict:
        """
        Get file stats in json format.

        :param path_depth: (optional) trim path, leaving a suffix of this size
        :return: (dict) file stats, including its path, number of lines, keywords and AST.
        """
        return {
            "path": self.get_str_path(path_depth),
            "ast": self._ast,
            "keyword_counts": self._keyword_counts,
            "n_lines": self.n_lines,
        }

    @staticmethod
    def get_essence_files_from_dir(  # noqa: PLR0913
        dir_path: str | Path,
        conjure_bin_path: str | Path,
        repo: Optional[Repo] = None,
        blocklist: Optional[Iterable[str]] = None,
        verbose: bool = False,
        exclude_regex: Optional[str] = None,
        max_n_files: Optional[int] = None,
    ):
        """
        Get Essence files contained in a given directory.

        :param dir_path: path to directory with essence files
        :param conjure_bin_path: a path to conjure binary
        :param blocklist: a list of Essence keywords to ignore
        :param verbose: Whether to print error messages
        :param exclude_regex: Exclude file paths that match this regular expression
        :param max_n_files: Maximum number of files to process
        :param repo: a Git repo that this directory belongs to (optional)
        """
        if verbose:
            print(f"Processing Essence files in {dir_path}...")
        counter = 0

        for fpath in tqdm(find_essence_files(dir_path, exclude_regex=exclude_regex)):
            try:
                if max_n_files is not None and counter >= max_n_files:
                    if verbose:
                        print(
                            f"Max number of files ({max_n_files}) reached, terminating...",
                        )
                    break

                file = EssenceFile(
                    fpath,
                    conjure_bin_path,
                    blocklist=blocklist,
                    repo=repo,
                )
                counter += 1
                yield file
            except Exception as e:
                if verbose:
                    print(f'Could not process file "{fpath}", throws exception: {e}')

        if verbose:
            print(f"{counter} Essence files processed!")
