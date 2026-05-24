conjure-oxide-debug solve model.essence --rewriter naive --comprehension-expander via-solver-ac --no-run-solver --rule-trace-aggregates trace-aggregates.txt >/dev/null
cat trace-aggregates.txt
rm trace-aggregates.txt
