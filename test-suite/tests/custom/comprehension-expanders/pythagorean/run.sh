# The guarded and unguarded Pythagorean-triples comprehensions from Figure 3 of
# "Solver Aided Expansion of Loops to Avoid Generate and Test" (ModRef 2025).
# Each formulation is checked at growing instance sizes. Every expander gets a
# five-second translation budget. Successful translations are compared with the
# first successful Minion model for that formulation and size.

set -eu

for formulation in guarded unguarded; do
    for n in 10 20 30 40 50 100 250; do
        for expander in native via-solver via-solver-ac; do
            rm -f \
                "model-$formulation-$n-$expander.minion" \
                "model-$formulation-$n-$expander.stderr" \
                "model-$formulation-$n-$expander.ok" \
                "model-$formulation-$n-$expander.failed"
        done
    done
done
rm -f parallel.joblog parallel.stderr

if parallel -j1 --no-notice --timeout 2 --joblog parallel.joblog \
        'if conjure-oxide solve --comprehension-expander {3} --no-run-solver --save-solver-input-file model-{1}-{2}-{3}.minion {1}.essence n{2}.param >/dev/null 2>model-{1}-{2}-{3}.stderr; then : >model-{1}-{2}-{3}.ok; else : >model-{1}-{2}-{3}.failed; fi' \
        ::: guarded unguarded \
        ::: 10 20 30 40 50 100 250 \
        ::: native via-solver via-solver-ac \
        2>parallel.stderr; then
    :
fi

jobs_completed=$(awk 'NR > 1 { count++ } END { print count + 0 }' parallel.joblog)
if [ "$jobs_completed" -ne 42 ]; then
    echo "GNU Parallel ran $jobs_completed of 42 jobs" >&2
    sed 's/^/  /' parallel.stderr >&2
    exit 1
fi

for formulation in guarded unguarded; do
    echo "== $formulation comprehension =="

    for n in 10 20 30 40 50 100 250; do
        echo "n=$n"
        reference_expander=

        for expander in native via-solver via-solver-ac; do
            result_prefix="model-$formulation-$n-$expander"

            if [ -f "$result_prefix.ok" ]; then
                if [ -z "$reference_expander" ]; then
                    reference_expander=$expander
                    lines=$(grep -c '' "$result_prefix.minion")
                    echo "  $expander: ok ($lines line model; reference)"
                elif cmp -s \
                        "model-$formulation-$n-$reference_expander.minion" \
                        "$result_prefix.minion"; then
                    echo "  $expander: ok (agrees with $reference_expander)"
                else
                    echo "  $expander: DIFFERENT from $reference_expander"
                    diff \
                        "model-$formulation-$n-$reference_expander.minion" \
                        "$result_prefix.minion"
                fi
            elif [ -f "$result_prefix.failed" ]; then
                echo "  $expander: failed" >&2
                sed 's/^/    /' "$result_prefix.stderr" >&2
                exit 1
            else
                echo "  $expander: timeout"
            fi
        done
    done
done

for formulation in guarded unguarded; do
    for n in 10 20 30 40 50 100 250; do
        for expander in native via-solver via-solver-ac; do
            rm -f \
                "model-$formulation-$n-$expander.minion" \
                "model-$formulation-$n-$expander.stderr" \
                "model-$formulation-$n-$expander.ok" \
                "model-$formulation-$n-$expander.failed"
        done
    done
done
rm -f parallel.joblog parallel.stderr
