from pathlib import Path
from typing import Optional, Iterable, Tuple

from git import Repo

from stats.essence_file import EssenceFile
from stats.essence_keyword import EssenceKeyword
from utils.conjure import download_conjure
from utils.files import trim_path
from utils.git_utils import clone_or_pull, parse_repo_url

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
        essence_dir: Path,
        essence_repo_urls: Iterable[Tuple[str, str]],
        conjure_version: str = "latest",
        blocklist: Optional[list[KeywordName]] = None,
        exclude_regex: Optional[str] = None,
        max_n_files=10000,
    ):
        """
        Create a new EssenceStats object.

        :param conjure_dir: Path to a directory containing conjure binary
        :param conjure_repo_url: GitHub URL to download conjure release from
        :param essence_dir: Local repo with Essence example files
        :param essence_repo_urls: List of GitHub repos with Essence example files
        :param essence_branch: Branch to download essence files from (master by default)
        :param conjure_version: Version of conjure to install (latest by default)
        :param blocklist: Essence keywords to ignore
        """
        if blocklist is None:
            blocklist = []

        self._max_n_files = max_n_files
        self._exclude_regex = exclude_regex
        self._essence_dir = essence_dir
        self._essence_repos = []
        for url, branch in essence_repo_urls:
            repo_user, repo_name = parse_repo_url(url)
            repo_path = self._essence_dir / repo_user / repo_name
            repo = clone_or_pull(
                repo_path,
                url,
                branch,
            )
            self._essence_repos.append(repo)

        self._conjure_bin = download_conjure(
            conjure_dir, repository_url=conjure_repo_url, version=conjure_version
        )

        self._blocklist = blocklist

        self._essence_keywords: dict[KeywordName, EssenceKeyword] = {}
        self._essence_files: dict[FilePath, EssenceFile] = {}

        self._update_stats()

    @property
    def essence_dir(self) -> Path:
        """Get path to essence examples dir."""
        return Path(self._essence_dir)

    @property
    def essence_repos(self) -> [Repo]:
        """Get a list of Repo objects - repositories with Essence files."""
        return self._essence_repos

    def get_essence_repo_names(self, depth=2):
        """Get Essence repos and paths to the repos, trimmed to a given depth."""
        return [trim_path(x.working_dir, depth) for x in self._essence_repos]

    def _update_stats(self):
        for repo in self._essence_repos:
            repo_dir = repo.working_dir

            files = list(
                EssenceFile.get_essence_files_from_dir(
                    repo_dir,
                    self._conjure_bin,
                    repo=repo,
                    blocklist=self._blocklist,
                    exclude_regex=self._exclude_regex,
                    max_n_files=self._max_n_files,
                )
            )

            for file in files:
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
