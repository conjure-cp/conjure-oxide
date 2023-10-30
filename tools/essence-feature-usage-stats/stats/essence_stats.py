from pathlib import Path
from typing import Optional

from stats.essence_keyword import EssenceKeyword
from stats.essence_file import EssenceFile
from utils.git import InvalidGitRemoteUrlError, sync_repo

KeywordName: type = str
FilePath: type = str

MOST_USED = "most-used"
AVG_USES = "avg-uses"
MOST_LINES = "most-lines"


class EssenceStats:
    """
    Class that stores stats for a given directory with
    """

    # ToDo use python getters / setters instead of java style,
    #  search: "python function as attribute" or ask Nik

    # ToDo some attrs should be private?

    def __init__(  # noqa: PLR0913
        self,
        essence_dir: str,
        conjure_bin: str,
        remote_repo_url=None,
        remote_branch="master",
        remote_name="origin",
        blocklist: Optional[list[KeywordName]] = None,
    ):
        if blocklist is None:
            blocklist = []
        self.essence_dir = Path(essence_dir).resolve()
        self.conjure_bin = Path(conjure_bin).resolve()

        self.blocklist = blocklist

        self.essence_keywords: dict[KeywordName, EssenceKeyword] = {}
        self.essence_files: dict[FilePath, EssenceFile] = {}

        self._remote_repo_url = remote_repo_url
        self._remote_branch = remote_branch
        self._remote_name = remote_name
        self._repo = None

        if remote_repo_url is not None:
            self.sync_repo(remote_repo_url, remote_branch, remote_name)

        # just incase its a local file
        # normally sync_repo calls this for us
        self._update_stats()

    def _update_stats(self):
        for file in EssenceFile.get_essence_files_from_dir(
            self.essence_dir,
            self.conjure_bin,
            blocklist=self.blocklist,
        ):
            self.essence_files[file.get_str_path()] = file

            for keyword in file.keywords:
                if keyword not in self.essence_keywords:
                    self.essence_keywords[keyword] = EssenceKeyword(keyword)
                self.essence_keywords[keyword].add_file(file)

    def sync_repo(
        self,
        remote_repo_url: Optional[str] = None,
        remote_branch: Optional[str] = None,
        remote_name: Optional[str] = None,
    ) -> None:
        """
        Sync to an upstream git repository.
        If an upstream repository is not given, the upstream repo specified at EssenceStats creation is used.
        """
        if remote_repo_url is None:
            remote_repo_url = self._remote_repo_url
        if remote_branch is None:
            remote_branch = self._remote_branch
        if remote_name is None:
            remote_name = self._remote_name

        try:
            self._repo = sync_repo(
                self.essence_dir,
                remote_repo_url,
                remote_name=remote_name,
                branch=remote_branch,
            )
            self._update_stats()

            self._remote_repo_url = remote_repo_url
            self._remote_branch = remote_branch
            self._remote_name = remote_name
        except Exception as e:
            raise InvalidGitRemoteUrlError(remote_repo_url) from e

    def get_essence_files(
        self,
        sort_mode: Optional[str] = None,
        reverse: bool = True,
    ) -> list[EssenceFile]:
        ans = list(self.essence_files.values())

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
        ans = list(self.essence_keywords.values())

        match sort_mode:
            case "most-used":
                ans.sort(key=lambda x: x.total_usages, reverse=reverse)
            case "avg-uses":
                ans.sort(key=lambda x: x.avg_usages, reverse=reverse)
            case _:
                pass

        return ans

    def get_stats_for_file(self, fpath: str) -> Optional[EssenceFile]:
        return self.essence_files.get(fpath, None)

    def get_stats_for_keyword(self, keyword: str) -> Optional[EssenceKeyword]:
        return self.essence_keywords.get(keyword, None)

    def as_json(self, path_depth=0) -> dict:
        return {
            "essence_files": [x.as_json(path_depth) for x in self.get_essence_files()],
            "essence_keywords": [
                x.as_json(path_depth) for x in self.get_essence_keywords()
            ],
        }
