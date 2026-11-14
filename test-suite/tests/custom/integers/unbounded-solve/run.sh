set -e
for solver in minion sat z3; do
    conjure-oxide --solver "$solver" solve model.essence --number-of-solutions=all
done
