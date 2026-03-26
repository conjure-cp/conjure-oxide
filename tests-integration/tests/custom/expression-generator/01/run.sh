conjure-oxide pretty model.essence --output-format="ast-json"
conjure-oxide-debug solve model.essence --no-run-solver --rule-trace-verbose trace-verbose.csv --rule-trace trace.csv

grep -c success trace-verbose.csv
wc -l trace-verbose.csv | awk '{print $1}'
rm trace-verbose.csv

cat trace.csv
rm trace.csv