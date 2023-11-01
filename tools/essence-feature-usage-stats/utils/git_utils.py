import shutil
from pathlib import Path
from typing import Tuple
from urllib.parse import urlsplit

from git import (
    RemoteProgress,
    Repo,
    GitCommandError,
)
from tqdm import tqdm


class InvalidGitRemoteUrlError(ValueError):
    """Raised when a git remote url is invalid."""

    def __init__(self, repo_url):  # noqa: D107
        super().__init__(f"Not a valid git repository url: {repo_url}")


class CloneProgress(RemoteProgress):
    """Progress bar for cloning a repo."""

    def __init__(self):  # noqa: D107
        super().__init__()
        self.pbar = tqdm(desc="Cloning repo: ", unit="%", ncols=100)

    def update(  # noqa: D102
        self,
        op_code,  # noqa: ARG002
        cur_count,
        max_count=None,
        message="",  # noqa: ARG002
    ):
        self.pbar.total = 100
        self.pbar.n = int((cur_count / max_count) * 100)
        self.pbar.refresh()


def is_git_repo(path: Path | str) -> bool:
    """Check whether a given directory is a git repository."""
    try:
        _ = Repo(path).git_dir
    except GitCommandError:
        return False
    else:
        return True


def clone_or_pull(
    directory_path: Path | str,
    remote_url: str,
    branch="master",
    remote_name="origin",
) -> Repo:
    """
    Clone a given GitHub repository to a given local directory, or pull latest changes if local repo exists.

    :param directory_path: local directory to use
    :param remote_url: remote repo url to pull from
    :param remote_name: name of the remote (origin by default)
    :param branch: branch of the remote repo to pull (master by default)
    """
    directory_path = Path(directory_path)
    directory_path.mkdir(exist_ok=True, parents=True)

    if directory_path.is_dir() and is_git_repo(directory_path):
        repo = Repo(directory_path)
        repo.remote(remote_name).pull()
    else:
        shutil.rmtree(directory_path)
        repo = Repo.clone_from(
            remote_url,
            directory_path,
            branch=branch,
            progress=CloneProgress(),
        )

    return repo


def parse_repo_url(repo_url: str) -> Tuple[str, str]:
    """
    Get the GitHub user and repo from a repo URL.

    :param repo_url: the GitHub repo URL
    :return: (user, repo)
    """
    if repo_url.startswith("http"):
        parsed_url = urlsplit(repo_url)
        path_components = parsed_url.path.strip("/").split("/")
        user, repo = path_components[:2]
        return user, repo

    elements = repo_url.split("/")
    return tuple(elements[:2])
