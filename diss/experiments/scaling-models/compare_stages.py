#!/usr/bin/env python3
"""
Compare rewriting and solution translation stages across all scaling-models experiments.

This script:
- Goes through all experiment directories (nqueens, bin_packing, pythagorean-triples)
- Reads their results.csv and combines them into a single dataframe
- Groups all rows into bins based on number of solutions
- Plots TWO comparison graphs:
  1. Rewriting time comparison (solver results only - N means different things across models)
  2. Solution translation time comparison
- Plus a dedicated N-Queens no-solver rewrite comparison

The plot style is similar to political comparison charts - each row shows a bin,
with dots for oxide and conjure connected by a line colored based on which is faster.
"""

import sys
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
import matplotlib.font_manager as fm
from pathlib import Path

# -- font setup --------------------------------------------------------------

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


def _set_title(ax, text):
    if _TITLE_FP is not None:
        ax.set_title(text, fontproperties=_TITLE_FP)
    else:
        ax.set_title(text, fontweight="bold", fontsize=14)


def load_all_results():
    """Load and combine results from all experiment directories (solver results only)."""
    script_dir = Path(__file__).parent
    experiment_dirs = ['nqueens', 'bin_packing', 'pythagorean-triples']

    all_data = []

    for exp_name in experiment_dirs:
        results_path = script_dir / exp_name / 'results.csv'
        if results_path.exists():
            df = pd.read_csv(results_path)
            df['experiment'] = exp_name
            all_data.append(df)
            print(f"Loaded {len(df)} rows from {exp_name}/results.csv")

    if not all_data:
        raise ValueError("No results found in any experiment directory")

    combined = pd.concat(all_data, ignore_index=True)
    return combined


def load_nqueens_no_solver():
    """Load only nqueens no-solver results for dedicated comparison."""
    script_dir = Path(__file__).parent
    results_path = script_dir / 'nqueens' / 'results-no-solver.csv'

    if not results_path.exists():
        raise ValueError(f"N-Queens no-solver results not found: {results_path}")

    df = pd.read_csv(results_path)
    df['experiment'] = 'nqueens'
    print(f"Loaded {len(df)} rows from nqueens/results-no-solver.csv")
    return df


def filter_valid_rows(df, for_translation=False):
    """
    Filter out invalid rows.
    For translation time, we need num_solutions > 0.
    For rewrite time, we just need rewrite_time_ms > 0.
    """
    df = df[df['rewrite_time_ms'] > 0].copy()

    if for_translation:
        df = df[df['num_solutions'] > 0].copy()
        df = df[df['solution_translation_time_ms'] >= 0].copy()

    return df


def create_solution_bins(df, num_bins=12):
    """
    Group data into bins based on number of solutions.
    Returns a dataframe with bin labels and aggregated times.
    """
    solution_counts = sorted(df['num_solutions'].unique())

    df['solution_bin'] = pd.qcut(
        df['num_solutions'].rank(method='dense'),
        q=min(num_bins, len(solution_counts)),
        labels=False,
        duplicates='drop'
    )

    bin_labels = {}
    for bin_id in df['solution_bin'].unique():
        bin_data = df[df['solution_bin'] == bin_id]
        min_sol = bin_data['num_solutions'].min()
        max_sol = bin_data['num_solutions'].max()
        if min_sol == max_sol:
            bin_labels[bin_id] = f"{min_sol:,}"
        else:
            bin_labels[bin_id] = f"{min_sol:,} - {max_sol:,}"

    df['bin_label'] = df['solution_bin'].map(bin_labels)

    return df, bin_labels


def create_n_bins(df, num_bins=12):
    """
    Group data into bins based on N (problem size).
    Used for no-solver results where num_solutions is 0.
    """
    n_values = sorted(df['N'].unique())

    df['n_bin'] = pd.qcut(
        df['N'].rank(method='dense'),
        q=min(num_bins, len(n_values)),
        labels=False,
        duplicates='drop'
    )

    bin_labels = {}
    for bin_id in df['n_bin'].unique():
        bin_data = df[df['n_bin'] == bin_id]
        min_n = bin_data['N'].min()
        max_n = bin_data['N'].max()
        if min_n == max_n:
            bin_labels[bin_id] = f"N={min_n}"
        else:
            bin_labels[bin_id] = f"N={min_n}-{max_n}"

    df['bin_label'] = df['n_bin'].map(bin_labels)

    return df, bin_labels


def aggregate_by_bin(df, bin_col, metric_col):
    """
    Aggregate times by system and bin.
    Returns mean and std for each group.
    """
    agg = df.groupby([bin_col, 'bin_label', 'system']).agg({
        metric_col: ['mean', 'std'],
    }).reset_index()

    agg.columns = [bin_col, 'bin_label', 'system', 'time_mean', 'time_std']

    return agg


def plot_comparison(agg_df, bin_col, output_path, title, xlabel):
    """
    Create the comparison plot.
    Each row shows a bin with dots for oxide and conjure,
    connected by a colored line (green if oxide < conjure, red otherwise).
    """
    # Pivot to get oxide and conjure side by side
    pivot = agg_df.pivot(index=[bin_col, 'bin_label'],
                         columns='system',
                         values='time_mean').reset_index()

    # Sort by bin
    pivot = pivot.sort_values(bin_col)

    # Convert to seconds
    pivot['oxide_s'] = pivot['oxide'] / 1000
    pivot['conjure_s'] = pivot['conjure'] / 1000

    # Set up the figure
    fig, ax = plt.subplots(figsize=(14, max(8, len(pivot) * 0.6)))

    # Plot each bin
    for i, (_, row) in enumerate(pivot.iterrows()):
        y = len(pivot) - 1 - i  # Reverse order so smallest is at top
        oxide_time = row['oxide_s']
        conjure_time = row['conjure_s']

        # Determine color based on which is faster
        if oxide_time < conjure_time:
            color = '#2ecc71'  # Green - oxide is faster
            alpha = 0.3
        else:
            color = '#e74c3c'  # Red - conjure is faster
            alpha = 0.3

        # Draw the connecting line with fill
        min_time = min(oxide_time, conjure_time)
        max_time = max(oxide_time, conjure_time)

        # Draw filled region between the two points
        ax.fill_betweenx([y - 0.15, y + 0.15],
                         min_time, max_time,
                         color=color, alpha=alpha)

        # Draw the line connecting the dots
        ax.plot([oxide_time, conjure_time], [y, y],
                color=color, linewidth=2, zorder=2)

        # Draw the dots
        ax.scatter(oxide_time, y, color='#3498db', s=100, zorder=3,
                   edgecolors='white', linewidths=1.5, label='Oxide' if i == 0 else '')
        ax.scatter(conjure_time, y, color='#9b59b6', s=100, zorder=3,
                   edgecolors='white', linewidths=1.5, label='Conjure' if i == 0 else '')

    # Customize the plot
    ax.set_yticks(range(len(pivot)))
    ax.set_yticklabels(pivot['bin_label'].iloc[::-1])
    ax.set_xlabel(xlabel, fontsize=12)
    ax.set_ylabel('Number of solutions (Buckets)', fontsize=12)
    _set_title(ax, title)

    # Use log scale for better visualization
    ax.set_xscale('log')

    # Add grid
    ax.grid(True, axis='x', alpha=0.3, linestyle='--')
    ax.axvline(x=1, color='gray', linestyle=':', alpha=0.5)  # 1 second reference line

    # Add legend in top right corner
    handles = [
        plt.scatter([], [], color='#3498db', s=100, edgecolors='white', linewidths=1.5),
        plt.scatter([], [], color='#9b59b6', s=100, edgecolors='white', linewidths=1.5),
        plt.Rectangle((0, 0), 1, 1, fc='#2ecc71', alpha=0.3),
        plt.Rectangle((0, 0), 1, 1, fc='#e74c3c', alpha=0.3)
    ]
    labels = ['Oxide', 'Conjure', 'Oxide faster', 'Conjure faster']
    ax.legend(handles, labels, loc='upper right', fontsize=10)

    plt.tight_layout()
    plt.savefig(output_path, dpi=150, bbox_inches='tight')
    print(f"Saved plot to {output_path}")
    plt.close()

    return pivot


def print_summary_stats(pivot, stage_name):
    """Print summary statistics comparing oxide and conjure."""
    print(f"\n{'='*60}")
    print(f"{stage_name.upper()} COMPARISON")
    print("="*60)

    pivot = pivot.copy()
    pivot['speedup'] = pivot['conjure_s'] / pivot['oxide_s']
    pivot['oxide_faster'] = pivot['oxide_s'] < pivot['conjure_s']

    for _, row in pivot.iterrows():
        faster = "OXIDE" if row['oxide_faster'] else "CONJURE"
        print(f"{row['bin_label']:>25}: Oxide={row['oxide_s']:.3f}s, "
              f"Conjure={row['conjure_s']:.3f}s, "
              f"Speedup={row['speedup']:.2f}x ({faster})")

    oxide_wins = pivot['oxide_faster'].sum()
    total = len(pivot)
    print(f"\nOxide faster in {oxide_wins}/{total} bins ({100*oxide_wins/total:.1f}%)")


def main():
    script_dir = Path(__file__).parent

    # Create output directory
    plots_dir = script_dir / 'combined-plots'
    plots_dir.mkdir(exist_ok=True)

    # ==========================================
    # 1. Rewriting comparison (solver results only)
    # ==========================================
    print("\n" + "="*60)
    print("LOADING DATA FOR REWRITING COMPARISON")
    print("="*60)

    df_rewrite = load_all_results()
    df_rewrite = filter_valid_rows(df_rewrite, for_translation=False)
    # Filter to only rows with valid num_solutions for binning
    df_rewrite = df_rewrite[df_rewrite['num_solutions'] > 0].copy()
    print(f"\nTotal valid rows for rewriting: {len(df_rewrite)}")

    df_rewrite, bin_labels = create_solution_bins(df_rewrite, num_bins=12)
    print(f"Created {len(bin_labels)} bins")

    agg_rewrite = aggregate_by_bin(df_rewrite, 'solution_bin', 'rewrite_time_ms')

    pivot_rewrite = plot_comparison(
        agg_rewrite, 'solution_bin',
        plots_dir / 'compare_rewrite_time.png',
        'Oxide vs Conjure: Rewrite Time',
        'Rewrite Time (seconds)'
    )
    print_summary_stats(pivot_rewrite, "Rewrite Time")

    # ==========================================
    # 2. Solution translation comparison (solver results only)
    # ==========================================
    print("\n" + "="*60)
    print("LOADING DATA FOR TRANSLATION COMPARISON")
    print("="*60)

    df_translation = load_all_results()
    df_translation = filter_valid_rows(df_translation, for_translation=True)
    print(f"\nTotal valid rows for translation: {len(df_translation)}")

    df_translation, bin_labels = create_solution_bins(df_translation, num_bins=12)
    print(f"Created {len(bin_labels)} bins")

    agg_translation = aggregate_by_bin(df_translation, 'solution_bin', 'solution_translation_time_ms')

    pivot_translation = plot_comparison(
        agg_translation, 'solution_bin',
        plots_dir / 'compare_translation_time.png',
        'Oxide vs Conjure: Solution Translation Time',
        'Solution Translation Time (seconds)'
    )
    print_summary_stats(pivot_translation, "Translation Time")

    # ==========================================
    # 3. N-Queens no-solver rewrite time comparison
    # ==========================================
    print("\n" + "="*60)
    print("N-QUEENS NO-SOLVER REWRITE TIME COMPARISON")
    print("="*60)

    df_nqueens = load_nqueens_no_solver()
    df_nqueens = df_nqueens[df_nqueens['rewrite_time_ms'] > 0].copy()
    print(f"\nTotal valid rows: {len(df_nqueens)}")

    df_nqueens, bin_labels = create_n_bins(df_nqueens, num_bins=15)
    print(f"Created {len(bin_labels)} bins")

    agg_nqueens = aggregate_by_bin(df_nqueens, 'n_bin', 'rewrite_time_ms')

    pivot_nqueens = plot_comparison(
        agg_nqueens, 'n_bin',
        plots_dir / 'compare_nqueens_rewrite.png',
        'Oxide vs Conjure: N-Queens Rewrite Time (No Solver)',
        'Rewrite Time (seconds)'
    )
    print_summary_stats(pivot_nqueens, "N-Queens Rewrite Time")

    print("\n" + "="*60)
    print("Done!")
    print("="*60)


if __name__ == '__main__':
    main()

