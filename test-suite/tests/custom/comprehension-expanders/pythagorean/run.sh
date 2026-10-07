# The guarded and unguarded Pythagorean-triples comprehensions from Figure 3 of
# "Solver Aided Expansion of Loops to Avoid Generate and Test" (ModRef 2025).
#
# Compare throughput rather than raw elapsed times: under the same per-model
# deadline, via-solver-ac must complete more formulations than via-solver, which
# must complete at least as many as native. This gives the performance ordering
# a wide tolerance for differences between local and CI machines.

set -eu

conjure_oxide=${CONJURE_OXIDE:-conjure-oxide}
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/conjure-oxide-pythagorean.XXXXXX")
trap 'rm -rf -- "$work_dir"' EXIT HUP INT TERM
export conjure_oxide work_dir

deadline=3
sizes="50 75 100"
expanders="native via-solver via-solver-ac"
formulations="guarded unguarded"
total_cases=6

if ! parallel --version 2>/dev/null | grep -q '^GNU parallel '; then
    echo "GNU Parallel is required" >&2
    exit 1
fi

if parallel -j1 --no-notice --timeout "$deadline" \
        --joblog "$work_dir/joblog" \
        'prefix="$work_dir/{1}-{2}-{3}"; if "$conjure_oxide" solve --comprehension-expander {1} --no-run-solver --save-solver-input-file "$prefix.minion" {2}.essence n{3}.param >/dev/null 2>"$prefix.stderr"; then : >"$prefix.ok"; else : >"$prefix.failed"; fi' \
        ::: $expanders \
        ::: $formulations \
        ::: $sizes \
        2>"$work_dir/parallel.stderr"; then
    :
fi

jobs_completed=$(awk 'NR > 1 { count++ } END { print count + 0 }' "$work_dir/joblog")
if [ "$jobs_completed" -ne 18 ]; then
    echo "GNU Parallel ran $jobs_completed of 18 jobs" >&2
    sed 's/^/  /' "$work_dir/parallel.stderr" >&2
    exit 1
fi

native_completed=0
via_solver_completed=0
via_solver_ac_completed=0

for expander in native via-solver via-solver-ac; do
    completed=0
    for formulation in $formulations; do
        for size in $sizes; do
            prefix="$work_dir/$expander-$formulation-$size"
            if [ -f "$prefix.ok" ]; then
                completed=$((completed + 1))
            elif [ -f "$prefix.failed" ]; then
                echo "$expander failed for $formulation at n=$size" >&2
                sed 's/^/  /' "$prefix.stderr" >&2
                exit 1
            fi
        done
    done

    case $expander in
        native) native_completed=$completed ;;
        via-solver) via_solver_completed=$completed ;;
        via-solver-ac) via_solver_ac_completed=$completed ;;
    esac
done

if [ "$via_solver_ac_completed" -le "$via_solver_completed" ] || \
        [ "$via_solver_completed" -lt "$native_completed" ]; then
    echo "unexpected completion counts within ${deadline}s:" >&2
    echo "  native: $native_completed/$total_cases" >&2
    echo "  via-solver: $via_solver_completed/$total_cases" >&2
    echo "  via-solver-ac: $via_solver_ac_completed/$total_cases" >&2
    exit 1
fi

echo "performance ordering holds: via-solver-ac > via-solver >= native (${deadline}s deadline)"
