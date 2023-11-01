import json
import subprocess
from os import PathLike
from pathlib import Path

import requests

from utils.files import download_and_extract, make_executable_recursive
from utils.git_utils import parse_repo_url

HTTP_OK = 200


def get_essence_file_ast(
    fpath: Path | PathLike[str] | str,
    conjure_bin_path: Path | PathLike[str] | str,
) -> dict:
    """
    Run the `conjure pretty` command line tool and get the parsed AST as a dict.

    :param conjure_bin_path: path to conjure binary
    :param fpath: path to an essence file
    :return: the Abstract Syntax Tree in json format (as a dict).
    """
    result = subprocess.run(
        [str(conjure_bin_path), "pretty", "--output-format=astjson", str(fpath)],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout)


def get_version(conjure_bin_path: Path | PathLike[str] | str) -> tuple[str, str]:
    """
    Get version from conjure binary.

    :param conjure_bin_path: path to conjure binary
    :return: tuple of (version, commit) - conjure version and git repo version (as given by conjure --version)
    """
    result = subprocess.run(
        [str(conjure_bin_path), "--version"],
        capture_output=True,
        text=True,
        check=True,
    )

    version, commit = None, None
    lines = result.stdout.split("\n")
    for line in lines:
        if "Release version" in line:
            version = "v" + line.removeprefix("Release version ")
        if "Repository version" in line:
            commit, *ts_parts = line.removeprefix("Repository version ").split()

    return version, commit


def get_release_id_by_version(repository_url: str, version: str) -> str | None:
    """Get release id for a specific release version of a repo from the GitHub API."""
    user, repo = parse_repo_url(repository_url)
    api_url = f"https://api.github.com/repos/{user}/{repo}/releases"
    response = requests.get(api_url)

    if response.status_code != HTTP_OK:
        print(f"Failed to get the latest release information from {api_url}")
    else:
        release_data = response.json()
        for release in release_data:
            if version in (release["name"], release["tag_name"]):
                return release[id]

    return None


def get_release_url(repository_url: str, version: str) -> str:
    """Build the GitHub API url for a specific release version of a repo."""
    user, repo = parse_repo_url(repository_url)

    if version != "latest":
        version = get_release_id_by_version(repository_url, version)

    return f"https://api.github.com/repos/{user}/{repo}/releases/{version}"


def get_conjure_zip_file_url(assets, version):
    """Get github relese asset for a release of conjure."""
    for asset in assets:
        if asset["name"] == f"conjure-{version}-linux.zip":
            return asset["browser_download_url"]
    return None


def download_conjure(
    output_dir: Path | PathLike[str] | str,
    version="latest",
    repository_url="https://github.com/conjure-cp/conjure",
):
    """
    Download conjure from GitHub and install the binary to a local directory.

    :param output_dir: local directory to download the conjure binary to
    :param version: Conjure release version ("latest" or "vX.Y.Z")
    :param repository_url: the GitHub repository URL
    """
    output_dir = Path(output_dir)
    if not output_dir.is_dir():
        print(f"Creating directory: {output_dir.resolve()}")
        output_dir.mkdir()

    print(
        f"Downloading Conjure release {version} from {repository_url} to {output_dir}",
    )

    api_url = get_release_url(repository_url, version)
    response = requests.get(api_url)

    if response.status_code != HTTP_OK:
        print(f"Failed to get the latest release information from {api_url}")
    else:
        release_data = response.json()
        version = release_data["tag_name"]
        assets = release_data["assets"]
        asset_file_url = get_conjure_zip_file_url(assets, version)

        download_and_extract(asset_file_url, output_dir)
        make_executable_recursive(output_dir)

        conjure_path = output_dir / f"conjure-{version}-linux" / "conjure"
        print(f"Conjure binary installed to {conjure_path.resolve()}")
        return conjure_path
    return None


if __name__ == "__main__":
    path = download_conjure("../conjure")
    print(get_version(path))
