conjure-oxide solve pythagorean05.essence --rewriter naive --comprehension-expander via-solver-ac --rule-trace-verbose trace.csv
cat trace.csv | grep success | wc -l
cat trace.csv | wc -l
rm trace.csv

conjure-oxide solve pythagorean10.essence --rewriter naive --comprehension-expander via-solver-ac --rule-trace-verbose trace.csv
cat trace.csv | grep success | wc -l
cat trace.csv | wc -l
rm trace.csv
