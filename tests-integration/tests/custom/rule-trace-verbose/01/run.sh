conjure-oxide solve pythagorean05.essence --rewriter naive --comprehension-expander via-solver-ac --rule-trace-verbose trace.csv --no-run-solver
grep -c success trace.csv
wc -l trace.csv | awk '{print $1}'
rm trace.csv

conjure-oxide solve pythagorean10.essence --rewriter naive --comprehension-expander via-solver-ac --rule-trace-verbose trace.csv --no-run-solver
grep -c success trace.csv
wc -l trace.csv | awk '{print $1}'
rm trace.csv
