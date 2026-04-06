#!/usr/bin/env python3
"""
Analyse Nested experiment results and produce plots.

Reads results.csv and generates (faceted by kind = matrix / record):
  1. N vs average e2e time (Oxide vs Conjure)
  2. N vs solution translation time (log scale)
  3. N vs rewrite time (log scale)
  4. N vs number of solutions (log scale)
  5. Phase breakdown per N, faceted by system and kind (log scale)
  6. Number of solutions vs e2e time (log-log)
  7. Number of solutions vs rewrite time (log-log)
  8. Number of solutions vs translation time (log-log)

Nested scaling varies N (nesting depth) with fixed inner size M.
"""

import sys
from pathlib import Path

import matplotlib
import matplotlib.pyplot as plt
import matplotlib.font_manager as fm
import numpy as np
import pandas as pd
import seaborn as sns

# -- paths -------------------------------------------------------------------

SCRIPT_DIR = Path(__file__).resolve().parent
CSV_PATH = SCRIPT_DIR / "results.csv"
PLOTS_DIR = SCRIPT_DIR / "plots"

# -- font setup --------------------------------------------------------------

# Cormorant Garamond Bold for titles, sans-serif for everything else.

_TITLE_FP = None
_font_dir = Path.home() / ".local" / "share" / "fonts"
_bold_ttf = _font_dir / "CormorantGaramond-Bold.ttf"
_var_ttf = _font_dir / "CormorantGaramond-VariableFont_wght.ttf"

if not _bold_ttf.exists() and _var_ttf.exists():
    try:
        from fontTools.ttLib import TTFont
        from fontTools.varLib import instancer

        font = TTFont(str(_var_ttf))
        instancer.instantiateVariableFont(
            font,
            {"wght": 700},
            inplace=True,
            overlap=instancer.OverlapMode.KEEP_AND_SET_FLAGS,
        )
        font.save(str(_bold_ttf))
    except Exception as exc:
        print(f"Warning: could not generate bold font: {exc}", file=sys.stderr)

if _bold_ttf.exists():
    fm.fontManager.addfont(str(_bold_ttf))
    _TITLE_FP = fm.FontProperties(fname=str(_bold_ttf), size=14)
else:
    print(
        "Warning: Cormorant Garamond Bold not found, titles will use sans-serif",
        file=sys.stderr,
    )

plt.rcParams.update(
    {
        "font.family": "sans-serif",
        "font.size": 10,
        "axes.labelsize": 10,
        "xtick.labelsize": 9,
        "ytick.labelsize": 9,
    }
)

_LEGEND_PROPS = {"family": "sans-serif", "size": 8}


def _set_title(ax, text):
    if _TITLE_FP is not None:
        ax.set_title(text, fontproperties=_TITLE_FP)
    else:
        ax.set_title(text, fontweight="bold", fontsize=14)


def _set_suptitle(fig, text):
    if _TITLE_FP is not None:
        fig.suptitle(text, fontproperties=_TITLE_FP, y=1.02)
    else:
        fig.suptitle(text, fontweight="bold", fontsize=14, y=1.02)


# -- colour scheme -----------------------------------------------------------
# Pastel palette matching the LaTeX document.

G_CONJURE = (0.75, 1.0, 1.0)  # cyan!25
G_SAVILEROW = (0.9375, 1.0, 0.75)  # lime!25
G_SOLVER = (0.89, 0.78, 1.0)  # violet!22
G_OXIDE = (1.0, 0.975, 0.79)  # Goldenrod!25

G_CONJURE_LINE = (0.0, 0.7, 0.7)
G_OXIDE_LINE = (0.8, 0.65, 0.0)

_PALETTE = {"Oxide": G_OXIDE_LINE, "Conjure": G_CONJURE_LINE}
_MARKERS = {"Oxide": "o", "Conjure": "s"}

# -- helpers -----------------------------------------------------------------


def load_data():
    df = pd.read_csv(CSV_PATH)

    # System label: oxide -> Oxide, conjure -> Conjure.
    df["System"] = df["system"].replace({"oxide": "Oxide", "conjure": "Conjure"})
    df["Kind"] = df["kind"].str.capitalize()

    # Oxide e2e = sum of components; Conjure e2e = conjure_solve_e2e_ms.
    oxide_mask = df["system"] == "oxide"
    df.loc[oxide_mask, "e2e_ms"] = (
        df.loc[oxide_mask, "rewrite_time_ms"]
        + df.loc[oxide_mask, "solver_time_ms"]
        + df.loc[oxide_mask, "solution_translation_time_ms"]
    )
    conjure_mask = df["system"] == "conjure"
    df.loc[conjure_mask, "e2e_ms"] = df.loc[conjure_mask, "conjure_solve_e2e_ms"]

    df["e2e_s"] = df["e2e_ms"] / 1000.0
    return df


def savefig(fig, name):
    PLOTS_DIR.mkdir(exist_ok=True)
    path = PLOTS_DIR / name
    fig.savefig(path, dpi=200, bbox_inches="tight")
    print(f"Saved {path}")


# -- plot 1: e2e time per kind ----------------------------------------------


def plot_e2e(df):
    kinds = df["Kind"].unique()
    fig, axes = plt.subplots(
        1, len(kinds), figsize=(6 * len(kinds), 4.5), sharey=True, squeeze=False
    )
    for ax, kind in zip(axes[0], kinds):
        sub = df[df["Kind"] == kind]
        sns.lineplot(
            data=sub,
            x="N",
            y="e2e_s",
            hue="System",
            style="System",
            markers=_MARKERS,
            dashes=False,
            palette=_PALETTE,
            linewidth=2,
            markersize=6,
            errorbar="sd",
            ax=ax,
        )
        _set_title(ax, kind)
        ax.set_xlabel("Nesting depth N")
        ax.set_ylabel("Average e2e time (s)")
        ax.legend(prop=_LEGEND_PROPS)
        ax.grid(True, alpha=0.3)
        sns.despine(ax=ax)
    _set_suptitle(fig, "Nested: End-to-End Time")
    fig.tight_layout()
    savefig(fig, "e2e_time.png")
    plt.close(fig)


# -- plot 2: solution translation time (log) per kind -----------------------


def plot_translation_time(df):
    kinds = df["Kind"].unique()
    fig, axes = plt.subplots(
        1, len(kinds), figsize=(6 * len(kinds), 4.5), sharey=True, squeeze=False
    )
    for ax, kind in zip(axes[0], kinds):
        sub = df[df["Kind"] == kind]
        sns.lineplot(
            data=sub,
            x="N",
            y="solution_translation_time_ms",
            hue="System",
            style="System",
            markers=_MARKERS,
            dashes=False,
            palette=_PALETTE,
            linewidth=2,
            markersize=6,
            errorbar="sd",
            ax=ax,
        )
        ax.set_yscale("log")
        _set_title(ax, kind)
        ax.set_xlabel("Nesting depth N")
        ax.set_ylabel("Solution translation time (ms, log)")
        ax.legend(prop=_LEGEND_PROPS)
        ax.grid(True, alpha=0.3, which="both")
        sns.despine(ax=ax)
    _set_suptitle(fig, "Nested: Solution Translation Time")
    fig.tight_layout()
    savefig(fig, "translation_time_log.png")
    plt.close(fig)


# -- plot 3: rewrite time (log) per kind ------------------------------------


def plot_rewrite_time(df):
    kinds = df["Kind"].unique()
    fig, axes = plt.subplots(
        1, len(kinds), figsize=(6 * len(kinds), 4.5), sharey=True, squeeze=False
    )
    for ax, kind in zip(axes[0], kinds):
        sub = df[df["Kind"] == kind]
        sns.lineplot(
            data=sub,
            x="N",
            y="rewrite_time_ms",
            hue="System",
            style="System",
            markers=_MARKERS,
            dashes=False,
            palette=_PALETTE,
            linewidth=2,
            markersize=6,
            errorbar="sd",
            ax=ax,
        )
        ax.set_yscale("log")
        _set_title(ax, kind)
        ax.set_xlabel("Nesting depth N")
        ax.set_ylabel("Rewrite time (ms, log)")
        ax.legend(prop=_LEGEND_PROPS)
        ax.grid(True, alpha=0.3, which="both")
        sns.despine(ax=ax)
    _set_suptitle(fig, "Nested: Rewrite Time")
    fig.tight_layout()
    savefig(fig, "rewrite_time_log.png")
    plt.close(fig)


# -- plot 4: number of solutions per kind -----------------------------------


def plot_num_solutions(df):
    oxide = df[df["System"] == "Oxide"]
    kinds = oxide["Kind"].unique()
    fig, axes = plt.subplots(
        1, len(kinds), figsize=(6 * len(kinds), 4.5), sharey=True, squeeze=False
    )
    for ax, kind in zip(axes[0], kinds):
        sub = oxide[oxide["Kind"] == kind]
        sns.barplot(
            data=sub,
            x="N",
            y="num_solutions",
            color=G_SOLVER,
            edgecolor="grey",
            linewidth=0.5,
            errorbar=None,
            ax=ax,
        )
        ax.set_yscale("log")
        _set_title(ax, kind)
        ax.set_xlabel("Nesting depth N")
        ax.set_ylabel("Number of solutions (log)")
        ax.grid(True, axis="y", alpha=0.3, which="both")
        sns.despine(ax=ax)
    _set_suptitle(fig, "Nested: Problem Scale")
    fig.tight_layout()
    savefig(fig, "num_solutions.png")
    plt.close(fig)


# -- plot 5: phase breakdown ------------------------------------------------

_PHASE_COLS = {
    "rewrite_time_ms": "Rewrite",
    "solver_time_ms": "Solve",
    "solution_translation_time_ms": "Translate",
}
_PHASE_PALETTE = {
    "Rewrite": G_SOLVER,
    "Solve": G_SAVILEROW,
    "Translate": G_CONJURE,
}


def plot_breakdown(df):
    # One catplot per kind to keep it readable.
    for kind in df["Kind"].unique():
        sub = df[df["Kind"] == kind]
        melted = sub.melt(
            id_vars=["System", "N"],
            value_vars=list(_PHASE_COLS.keys()),
            var_name="Phase",
            value_name="time_ms",
        )
        melted["Phase"] = melted["Phase"].map(_PHASE_COLS)
        melted["time_s"] = melted["time_ms"] / 1000.0

        g = sns.catplot(
            data=melted,
            x="N",
            y="time_s",
            hue="Phase",
            col="System",
            kind="bar",
            errorbar="sd",
            hue_order=["Solve", "Rewrite", "Translate"],
            palette=_PHASE_PALETTE,
            edgecolor="grey",
            linewidth=0.5,
            height=5,
            aspect=1.3,
            legend="auto",
            facet_kws={"gridspec_kws": {"wspace": 0.1}},
        )
        g.set(yscale="log")
        g.set_axis_labels("Nesting depth N", "Time (s, log scale)")
        if _TITLE_FP is not None:
            g.figure.suptitle(
                f"Nested ({kind}): Phase Breakdown",
                fontproperties=_TITLE_FP,
                y=1.02,
            )
        else:
            g.figure.suptitle(
                f"Nested ({kind}): Phase Breakdown",
                fontweight="bold",
                fontsize=14,
                y=1.02,
            )
        sns.move_legend(
            g, "upper left", bbox_to_anchor=(0.08, 1.05), prop=_LEGEND_PROPS, title=None
        )
        savefig(g.figure, f"breakdown_{kind.lower()}.png")
        plt.close(g.figure)


# -- plot 6: e2e time vs number of solutions --------------------------------


def plot_e2e_vs_solutions(df):
    kinds = df["Kind"].unique()
    fig, axes = plt.subplots(
        1, len(kinds), figsize=(6 * len(kinds), 4.5), sharey=True, squeeze=False
    )
    for ax, kind in zip(axes[0], kinds):
        sub = df[df["Kind"] == kind]
        sns.lineplot(
            data=sub,
            x="num_solutions",
            y="e2e_s",
            hue="System",
            style="System",
            markers=_MARKERS,
            dashes=False,
            palette=_PALETTE,
            linewidth=2,
            markersize=6,
            errorbar="sd",
            ax=ax,
        )
        ax.set_xscale("log")
        ax.set_yscale("log")
        _set_title(ax, kind)
        ax.set_xlabel("Number of solutions (log)")
        ax.set_ylabel("Average e2e time (s, log)")
        ax.legend(prop=_LEGEND_PROPS)
        ax.grid(True, alpha=0.3, which="both")
        sns.despine(ax=ax)
    _set_suptitle(fig, "Nested: E2E Time vs Solutions")
    fig.tight_layout()
    savefig(fig, "e2e_vs_solutions.png")
    plt.close(fig)


# -- plot 7: rewrite time vs number of solutions ----------------------------


def plot_rewrite_vs_solutions(df):
    kinds = df["Kind"].unique()
    fig, axes = plt.subplots(
        1, len(kinds), figsize=(6 * len(kinds), 4.5), sharey=True, squeeze=False
    )
    for ax, kind in zip(axes[0], kinds):
        sub = df[df["Kind"] == kind]
        sns.lineplot(
            data=sub,
            x="num_solutions",
            y="rewrite_time_ms",
            hue="System",
            style="System",
            markers=_MARKERS,
            dashes=False,
            palette=_PALETTE,
            linewidth=2,
            markersize=6,
            errorbar="sd",
            ax=ax,
        )
        ax.set_xscale("log")
        ax.set_yscale("log")
        _set_title(ax, kind)
        ax.set_xlabel("Number of solutions (log)")
        ax.set_ylabel("Rewrite time (ms, log)")
        ax.legend(prop=_LEGEND_PROPS)
        ax.grid(True, alpha=0.3, which="both")
        sns.despine(ax=ax)
    _set_suptitle(fig, "Nested: Rewrite Time vs Solutions")
    fig.tight_layout()
    savefig(fig, "rewrite_vs_solutions.png")
    plt.close(fig)


# -- plot 8: translation time vs number of solutions ------------------------


def plot_translation_vs_solutions(df):
    kinds = df["Kind"].unique()
    fig, axes = plt.subplots(
        1, len(kinds), figsize=(6 * len(kinds), 4.5), sharey=True, squeeze=False
    )
    for ax, kind in zip(axes[0], kinds):
        sub = df[df["Kind"] == kind]
        sns.lineplot(
            data=sub,
            x="num_solutions",
            y="solution_translation_time_ms",
            hue="System",
            style="System",
            markers=_MARKERS,
            dashes=False,
            palette=_PALETTE,
            linewidth=2,
            markersize=6,
            errorbar="sd",
            ax=ax,
        )
        ax.set_xscale("log")
        ax.set_yscale("log")
        _set_title(ax, kind)
        ax.set_xlabel("Number of solutions (log)")
        ax.set_ylabel("Translation time (ms, log)")
        ax.legend(prop=_LEGEND_PROPS)
        ax.grid(True, alpha=0.3, which="both")
        sns.despine(ax=ax)
    _set_suptitle(fig, "Nested: Translation Time vs Solutions")
    fig.tight_layout()
    savefig(fig, "translation_vs_solutions.png")
    plt.close(fig)


# -- main --------------------------------------------------------------------


def main():
    if not CSV_PATH.exists():
        print(
            f"Error: {CSV_PATH} not found. Run the experiment first.", file=sys.stderr
        )
        sys.exit(1)

    df = load_data()

    plot_e2e(df)
    plot_translation_time(df)
    plot_rewrite_time(df)
    plot_num_solutions(df)
    plot_breakdown(df)
    plot_e2e_vs_solutions(df)
    plot_rewrite_vs_solutions(df)
    plot_translation_vs_solutions(df)

    print("Done.")


if __name__ == "__main__":
    main()
