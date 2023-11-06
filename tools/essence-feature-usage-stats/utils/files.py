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


def make_executable_recursive(path: Path):
    """Recursively make files in a directory executable."""
    if path.is_file():
        path.chmod(path.stat().st_mode | 0o111)
    else:
        for item in path.iterdir():
            make_executable_recursive(item)  # Recursively process subdirectories


def download_and_extract(download_url: str, dir_path: Path | str) -> Path | None:
    """Download and extract a file from a URL to a local directory."""
    file_path = None
    zip_path = dir_path / "temp.zip"
    download_file(download_url, zip_path)

    with zipfile.ZipFile(zip_path, "r") as zip_ref:
        conjure_names = list(
            filter(lambda x: x.startswith("conjure"), zip_ref.namelist())
        )

        if not conjure_names:
            raise ValueError("No conjure files found in release!")  # noqa: TRY003

        conjure_root = conjure_names[0]
        for name in conjure_names:
            if all(x.startswith(name) for x in conjure_names):
                conjure_root = name

        file_path = Path(zip_ref.extract(conjure_root, dir_path))
        zip_ref.extractall(dir_path)

    zip_path.unlink()
    return file_path


def find_file(directory_path: Path, target_file_name: str) -> Path | None:
    """Recursively search directory for a given file."""
    directory_path = Path(directory_path)

    if directory_path.is_file() and directory_path.name == target_file_name:
        return directory_path

    if directory_path.is_dir():
        for file in directory_path.iterdir():
            result = find_file(file, target_file_name)
            if result is not None:
                return result

    return None  # File not found in the directory or its subdirectories
