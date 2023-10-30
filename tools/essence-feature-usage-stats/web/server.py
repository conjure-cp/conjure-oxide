from flask import Flask, render_template, request

from stats.essence_stats import EssenceStats


def create_server(stats: EssenceStats):
    app = Flask(__name__)

    @app.route("/")
    @app.route("/index.html")
    def index():
        # ToDo cache Essence ast generation etc
        n_keywords = request.args.get("n_keywords", default=5, type=int)
        return render_template(
            "index.html", data={"essence_stats": stats, "n_keywords": n_keywords}
        )

    return app
