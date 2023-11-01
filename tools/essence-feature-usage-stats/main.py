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
ESSENCE_EXAMPLES_REPO = os.getenv("ESSENCE_EXAMPLES_REPO")

jinja_env = Environment(
    loader=FileSystemLoader(Path("web/templates")),
    autoescape=select_autoescape(),
)

if __name__ == "__main__":
    stats = EssenceStats(
        CONJURE_DIR,
        CONJURE_REPO,
        ESSENCE_DIR,
        ESSENCE_EXAMPLES_REPO,
        "master",
        KEYWORD_BLOCKLIST,
    )

    timestamp = datetime.datetime.now().strftime("%d.%m.%Y - %H:%M")
    template = jinja_env.get_template("index.html")
    html = template.render(
        data={
            "essence_stats": stats,
            "n_keywords": 5,
            "css_path": "web/static/styles.css",
            "script_path": "web/static/script.js",
            "timestamp": timestamp,
        },
    )

    with OUTPUT_PATH.open("w") as f:
        f.write(html)
        f.close()
