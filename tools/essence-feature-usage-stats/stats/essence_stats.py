from pathlib import Path
from typing import Optional

from stats.essence_file import EssenceFile
from stats.essence_keyword import EssenceKeyword
from utils.conjure import download_conjure
from utils.git_utils import clone_or_pull

KeywordName: type = str
FilePath: type = str

MOST_USED = "most-used"
AVG_USES = "avg-uses"
MOST_LINES = "most-lines"


class EssenceStats:
    """Class that stores stats for a given directory with."""

    def __init__(  # noqa: PLR0913
        self,
        conjure_dir: Path,
        conjure_repo_url: str,
        essence_repo_dir: Path,
        essence_repo_url: str,
        essence_branch="master",
        blocklist: Optional[list[KeywordName]] = None,
    ):
        """
        Create a new EssenceStats object.

        :param conjure_dir: Path to a directory containing conjure binary
        :param conjure_repo_url: GitHub URL to download conjure release from
        :param essence_repo_dir: Local repo with Essence example files
        :param essence_repo_url: GitHub repo with Essence example files
        :param essence_branch: Branch to download essence files from (master by default)
        :param blocklist: Essence keywords to ignore
        """
        if blocklist is None:
            blocklist = []

        self._essence_repo = clone_or_pull(
            essence_repo_dir,
            essence_repo_url,
            essence_branch,
        )

        self._conjure_bin = download_conjure(
            conjure_dir,
            repository_url=conjure_repo_url,
        )

        self._blocklist = blocklist

        self._essence_keywords: dict[KeywordName, EssenceKeyword] = {}
        self._essence_files: dict[FilePath, EssenceFile] = {}

        self._update_stats()

    @property
    def essence_dir(self) -> Path:
        """Get path to essence examples dir."""
        return Path(self._essence_repo.working_dir)

    def _update_stats(self):
        for file in EssenceFile.get_essence_files_from_dir(
            self.essence_dir,
            self._conjure_bin,
            blocklist=self._blocklist,
        ):
            self._essence_files[file.get_str_path()] = file

            for keyword in file.keywords:
                if keyword not in self._essence_keywords:
                    self._essence_keywords[keyword] = EssenceKeyword(keyword)
                self._essence_keywords[keyword].add_file(file)

    def get_essence_files(
        self,
        sort_mode: Optional[str] = None,
        reverse: bool = True,
    ) -> list[EssenceFile]:
        """Get a list of all essence example files."""
        ans = list(self._essence_files.values())

        match sort_mode:
            case "most-lines":
                ans.sort(key=lambda x: x.n_lines, reverse=reverse)
            case _:
                pass

        return ans

    def get_essence_keywords(
        self,
        sort_mode: Optional[str] = None,
        reverse: bool = True,
    ) -> list[EssenceKeyword]:
        """Get all essence keywords used across all files."""
        ans = list(self._essence_keywords.values())

        match sort_mode:
            case "most-used":
                ans.sort(key=lambda x: x.total_usages, reverse=reverse)
            case "avg-uses":
                ans.sort(key=lambda x: x.avg_usages, reverse=reverse)
            case _:
                pass

        return ans

    def get_stats_for_file(self, fpath: str) -> Optional[EssenceFile]:
        """Get stats for a specific file."""
        return self._essence_files.get(fpath, None)

    def get_stats_for_keyword(self, keyword: str) -> Optional[EssenceKeyword]:
        """Get stats for a specific keyword."""
        return self._essence_keywords.get(keyword, None)

    def as_json(self, path_depth=0) -> dict:
        """Get the essence stats as a JSON dictionary."""
        return {
            "essence_files": [x.as_json(path_depth) for x in self.get_essence_files()],
            "essence_keywords": [
                x.as_json(path_depth) for x in self.get_essence_keywords()
            ],
        }
