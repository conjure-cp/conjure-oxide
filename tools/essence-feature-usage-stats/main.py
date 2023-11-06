import datetime
import os
from pathlib import Path

from dotenv import load_dotenv
from jinja2 import Environment, FileSystemLoader, select_autoescape

from stats.essence_stats import EssenceStats

ENV_PATH = Path("./.env").resolve()
load_dotenv(dotenv_path=ENV_PATH)

KEYWORD_BLOCKLIST = [x.strip() for x in os.getenv("KEYWORD_BLOCKLIST").split(",")]
ESSENCE_DIR = Path(os.getenv("ESSENCE_DIR"))
CONJURE_DIR = Path(os.getenv("CONJURE_DIR"))
OUTPUT_PATH = Path(os.getenv("OUTPUT_PATH"))
CONJURE_REPO = os.getenv("CONJURE_REPO")

CONJURE_VERSION = os.getenv("CONJURE_VERSION")
if CONJURE_VERSION is None:
    CONJURE_VERSION = "latest"

ESSENCE_EXAMPLES_REPO = os.getenv("ESSENCE_EXAMPLES_REPO")

jinja_env = Environment(
    loader=FileSystemLoader(Path("web/templates")),
    autoescape=select_autoescape(),
)

if __name__ == "__main__":
    stats = EssenceStats(
        conjure_dir=CONJURE_DIR,
        conjure_repo_url=CONJURE_REPO,
        essence_dir=ESSENCE_DIR,
        essence_repo_urls=[ESSENCE_EXAMPLES_REPO],
        essence_branch="master",
        conjure_version="v2.4.1",
        blocklist=KEYWORD_BLOCKLIST,
    )

    timestamp = datetime.datetime.now().strftime("%d.%m.%Y - %H:%M")
    template = jinja_env.get_template("index.html")
    html = template.render(
        data={
            "essence_stats": stats,
            "n_keywords": 200,
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
