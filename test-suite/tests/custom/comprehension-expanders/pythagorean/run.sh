# The guarded and unguarded Pythagorean-triples comprehensions from Figure 3 of
# "Solver Aided Expansion of Loops to Avoid Generate and Test" (ModRef 2025).
# Each formulation is checked at small and large instance sizes. Every expander
# gets a five-second translation budget, with expected outcomes chosen well away
# from the machine-dependent crossover points.

set -eu

conjure_oxide=${CONJURE_OXIDE:-conjure-oxide}
export conjure_oxide

sizes="10 500"
expanders="native via-solver via-solver-ac auto"

for formulation in guarded unguarded; do
    for n in $sizes; do
        for expander in $expanders; do
            rm -f \
                "model-$formulation-$n-$expander.minion" \
                "model-$formulation-$n-$expander.stderr" \
                "model-$formulation-$n-$expander.ok" \
                "model-$formulation-$n-$expander.failed"
        done
    done
done
rm -f parallel.joblog parallel.stderr

if parallel -j1 --no-notice --timeout 5 --joblog parallel.joblog \
        'if "$conjure_oxide" solve --comprehension-expander {3} --no-run-solver --save-solver-input-file model-{1}-{2}-{3}.minion {1}.essence n{2}.param >/dev/null 2>model-{1}-{2}-{3}.stderr; then : >model-{1}-{2}-{3}.ok; else : >model-{1}-{2}-{3}.failed; fi' \
        ::: guarded unguarded \
        ::: 10 500 \
        ::: native via-solver via-solver-ac auto \
        2>parallel.stderr; then
    :
fi

jobs_completed=$(awk 'NR > 1 { count++ } END { print count + 0 }' parallel.joblog)
if [ "$jobs_completed" -ne 16 ]; then
    echo "GNU Parallel ran $jobs_completed of 16 jobs" >&2
    sed 's/^/  /' parallel.stderr >&2
    exit 1
fi

for formulation in guarded unguarded; do
    echo "== $formulation comprehension =="

    for n in $sizes; do
        reference_expander=via-solver-ac
        reference_prefix="model-$formulation-$n-$reference_expander"
        if [ ! -f "$reference_prefix.ok" ]; then
            echo "  $reference_expander did not complete for $formulation at n=$n" >&2
            sed 's/^/    /' "$reference_prefix.stderr" >&2
            exit 1
        fi

        lines=$(grep -c '' "$reference_prefix.minion")
        echo "n=$n ($lines line model)"

        for expander in $expanders; do
            result_prefix="model-$formulation-$n-$expander"
            expected_status=ok
            if [ "$n" = 500 ] && { [ "$expander" = native ] || \
                    { [ "$formulation" = unguarded ] && [ "$expander" = via-solver ]; }; }; then
                expected_status=timeout
            fi

            if [ -f "$result_prefix.ok" ]; then
                actual_status=ok
                if [ "$expander" != "$reference_expander" ] && ! cmp -s \
                        "$reference_prefix.minion" "$result_prefix.minion"; then
                    echo "  $expander: DIFFERENT from $reference_expander"
                    diff "$reference_prefix.minion" "$result_prefix.minion"
                    exit 1
                fi
            elif [ -f "$result_prefix.failed" ]; then
                echo "  $expander: failed" >&2
                sed 's/^/    /' "$result_prefix.stderr" >&2
                exit 1
            else
                actual_status=timeout
            fi

            if [ "$actual_status" != "$expected_status" ]; then
                echo "  $expander: expected $expected_status, got $actual_status" >&2
                exit 1
            fi
            echo "  $expander: $actual_status"
        done
    done
done

for formulation in guarded unguarded; do
    for n in $sizes; do
        for expander in $expanders; do
            rm -f \
                "model-$formulation-$n-$expander.minion" \
                "model-$formulation-$n-$expander.stderr" \
                "model-$formulation-$n-$expander.ok" \
                "model-$formulation-$n-$expander.failed"
        done
    done
done
rm -f parallel.joblog parallel.stderr
