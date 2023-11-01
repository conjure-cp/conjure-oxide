import os
import zipfile
from pathlib import Path

import requests


def count_lines(fpath: str | Path) -> int:
    """
    Count the number of lines in a file.

    :param fpath: path to the file
    :return: int, the number of lines.
    """
    fpath = Path(fpath)
    with fpath.open("r") as f:
        return sum(1 for _ in f)


def trim_path(input_path: os.PathLike | Path | str, num_elements=0) -> str:
    """
    Normalize path and get last N elements from the end of the path (returns whole path if num_elements is 0).

    :param input_path: the path
    :param num_elements: last N elements to return
    :return: whole path or a part of it (str).
    """
    input_path = os.path.normpath(str(input_path))

    if num_elements == 0:
        return input_path

    path_elements = input_path.split(os.path.sep)
    num_elements = min(
        num_elements,
        len(path_elements),
    )  # Ensure num_elements is not greater than the length of the path
    return os.path.sep.join(
        path_elements[-num_elements:],
    )  # Join the last num_elements elements to form the trimmed path


def download_file(download_url: str, file_path: Path | str):
    """Download a file from a URL to a local file."""
    file_path = Path(file_path)

    print(f"Downloading from {download_url} to {file_path.resolve()}...")
    file_path.touch(exist_ok=True)

    response = requests.get(download_url, stream=True)
    with file_path.open("wb") as file:
        for chunk in response.iter_content(chunk_size=8192):
            if chunk:
                file.write(chunk)


def make_executable_recursive(directory_path):
    """Recursively make files in a directory executable."""
    for item in directory_path.iterdir():
        if item.is_file():
            item.chmod(item.stat().st_mode | 0o111)  # Add execute permission for files
        elif item.is_dir():
            make_executable_recursive(item)  # Recursively process subdirectories


def download_and_extract(download_url: str, dir_path: Path | str):
    """Download and extract a file from a URL to a local directory."""
    temp_path = dir_path / "temp.zip"
    download_file(download_url, temp_path)

    with zipfile.ZipFile(temp_path, "r") as zip_ref:
        zip_ref.extractall(dir_path)

    temp_path.unlink()
