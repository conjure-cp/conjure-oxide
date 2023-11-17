from os import PathLike
from pathlib import Path

from stats.essence_stats import EssenceStats


def make_table_data(stats: EssenceStats, path_depth: int = 4):
    """Convert EssenceStats to lines of a table."""
    keywords = stats.get_essence_keywords(sort_mode="most-used")
    files = stats.get_essence_files(sort_mode="most-lines", reverse=False)

    # CSV File headings
    yield ["EssenceFile", "LOC", "Repo", *[keyword.name for keyword in keywords]]

    for file in files:
        yield [
            file.get_str_path(path_depth),
            file.n_lines,
            file.get_repo_name(depth=2),
            *[file.get_uses(keyword.name) for keyword in keywords],
        ]


def make_csv_lines(stats: EssenceStats, delimiter: str = ",", path_depth: int = 4):
    """Utility function to convert EssenceStats to CSV file lines."""  # noqa: D401
    for line in make_table_data(stats, path_depth=path_depth):
        yield delimiter.join([str(x) for x in line]) + "\n"


def write_csv(
    stats: EssenceStats,
    fpath: Path | PathLike[str] | str,
    delimiter: str = ",",
    path_depth: int = 4,
):
    """Write essence stats to csv file."""
    fpath = Path(fpath)

    if fpath.exists() and not fpath.is_file():
        raise ValueError("Must be a valid file!")  # noqa: TRY003

    with fpath.open("w") as file:
        file.writelines(
            make_csv_lines(stats, delimiter=delimiter, path_depth=path_depth)
        )
