conjure-oxide solve pythagorean05.essence --rewriter baseline --comprehension-expander via-solver-ac --rule-attempt-trace trace.csv --no-run-solver
grep -c success trace.csv
wc -l trace.csv | awk '{print $1}'
rm trace.csv

conjure-oxide solve pythagorean10.essence --rewriter baseline --comprehension-expander via-solver-ac --rule-attempt-trace trace.csv --no-run-solver
grep -c success trace.csv
wc -l trace.csv | awk '{print $1}'
rm trace.csv
