import datetime
import os
from pathlib import Path

from dotenv import load_dotenv
from jinja2 import Environment, FileSystemLoader, select_autoescape

from stats.essence_stats import EssenceStats
from utils.misc import parse_essence_repos
from web.csv import write_csv

ENV_PATH = Path("./.env").resolve()
load_dotenv(dotenv_path=ENV_PATH)

KEYWORD_BLOCKLIST = [
    x.strip().replace('"', "") for x in os.getenv("KEYWORD_BLOCKLIST").split(",")
]

ESSENCE_DIR = Path(os.getenv("ESSENCE_DIR"))
CONJURE_DIR = Path(os.getenv("CONJURE_DIR"))
OUTPUT_PATH = Path(os.getenv("OUTPUT_PATH"))
CONJURE_REPO = os.getenv("CONJURE_REPO")
MAX_N_FILES = int(os.getenv("MAX_N_FILES", "200"))
MAX_N_KEYWORDS = int(os.getenv("MAX_N_KEYWORDS", "200"))
CONJURE_VERSION = os.getenv("CONJURE_VERSION", "latest")

EXCLUDE_REGEX = os.getenv("EXCLUDE_PATHS_REGEX")
if EXCLUDE_REGEX is not None:
    EXCLUDE_REGEX = EXCLUDE_REGEX.strip().replace('"', "")
    EXCLUDE_REGEX = rf"{EXCLUDE_REGEX}"

ESSENCE_FILE_REPOS = parse_essence_repos(os.getenv("ESSENCE_FILE_REPOS"))

jinja_env = Environment(
    loader=FileSystemLoader(Path("web/templates")),
    autoescape=select_autoescape(),
)

if __name__ == "__main__":
    stats = EssenceStats(
        conjure_dir=CONJURE_DIR,
        conjure_repo_url=CONJURE_REPO,
        essence_dir=ESSENCE_DIR,
        essence_repo_urls=ESSENCE_FILE_REPOS,
        conjure_version=CONJURE_VERSION,
        blocklist=KEYWORD_BLOCKLIST,
        exclude_regex=EXCLUDE_REGEX,
    )

    # write_csv(stats, "data.csv")

    timestamp = datetime.datetime.now().strftime("%d.%m.%Y - %H:%M")
    template = jinja_env.get_template("index.html")
    html = template.render(
        data={
            "essence_stats": stats,
            "n_keywords": MAX_N_KEYWORDS,
            "n_files": MAX_N_FILES,
            "css_path": "styles.css",
            "script_path": "script.js",
            "timestamp": timestamp,
        },
    )

    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    with OUTPUT_PATH.open("w") as f:
        f.write(html)
        f.close()
    print(f"Table created: {OUTPUT_PATH.resolve()}")
