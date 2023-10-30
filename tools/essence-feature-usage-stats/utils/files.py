import os
from pathlib import Path


def count_lines(fpath: str | Path) -> int:
    """
    Counts the number of lines in a file
    :param fpath: path to the file
    :return: int, the number of lines
    """
    fpath = Path(fpath)
    with fpath.open("r") as f:
        return sum(1 for _ in f)


def trim_path(input_path: os.PathLike | Path | str, num_elements=0) -> str:
    """
    Normalize path and get last N elements from the end of the path (returns whole path if num_elements is 0)
    :param input_path: the path
    :param num_elements: last N elements to return
    :return: whole path or a part of it (str)
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
