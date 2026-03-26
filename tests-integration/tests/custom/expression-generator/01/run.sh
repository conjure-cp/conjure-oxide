conjure-oxide pretty model.essence --output-format="ast-json"
conjure-oxide-debug solve model.essence --no-run-solver --rule-trace-verbose trace.csv
grep -c success trace.csv
wc -l trace.csv | awk '{print $1}'
rm trace.csv