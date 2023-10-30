import os
import shutil
from pathlib import Path

import git
from git import RemoteProgress, Repo
from tqdm import tqdm


class InvalidGitRemoteUrlError(ValueError):
    """This exception is raised when a git remote url is invalid."""

    def __init__(self, repo_url):
        super().__init__(f"Not a valid git repository url: {repo_url}")


class CloneProgress(RemoteProgress):
    def __init__(self):
        super().__init__()
        self.pbar = tqdm(desc="Cloning repo: ", unit="%", ncols=100)

    def update(self, op_code, cur_count, max_count=None, message=""):  # noqa: ARG002
        self.pbar.total = 100
        self.pbar.n = int((cur_count / max_count) * 100)
        self.pbar.refresh()


def sync_repo(
    directory_path: str | Path,
    repo_url,
    branch="master",
    remote_name="origin",
) -> Repo:
    """
    Given a directory and a remote repo, synchronise directory with the repo.
    That is:
    - If directory does not exist, clone remote repo to directory
    - If it exists, try to pull latest commit
    - If it is not a valid repo, delete directory and clone repo again
    :param directory_path: - path to directory
    :param repo_url: - url of remote git repo
    :param branch: - branch to use (master by default)
    :param remote_name: - remote name to use (origin by default)
    :return: - None
    """

    directory_path = Path(directory_path)

    if directory_path.exists() and len(os.listdir(directory_path)) == 0:
        # If it's an empty directory, remove it (and clone repo)
        directory_path.rmdir()

    if (
        not directory_path.exists()
    ):  # If the directory does not exist, clone the repository
        repo = git.Repo.clone_from(
            repo_url,
            directory_path,
            progress=CloneProgress(),
            branch=branch,
        )
        print(f"Cloned {repo_url} into {directory_path}")
        return repo

    try:  # If the directory exists, try to pull the latest changes
        repo = git.Repo(directory_path)
        origin = repo.remote(name=remote_name)
        origin.pull(branch, progress=CloneProgress())
        print(f"Pulled the latest changes for {repo_url} in {directory_path}")
    except git.exc.InvalidGitRepositoryError:
        # If the directory exists but is not a valid Git repository, remove it and clone again
        print(f"Removing invalid repository in {directory_path}")
        shutil.rmtree(directory_path)
        repo = git.Repo.clone_from(
            repo_url,
            directory_path,
            progress=CloneProgress(),
            branch=branch,
        )
        print(f"Cloned {repo_url} into {directory_path}")

    return repo
