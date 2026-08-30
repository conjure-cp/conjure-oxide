conjure-oxide pretty model.essence --output-format="ast-json"

echo ""
echo ""
echo ""

conjure-oxide-debug solve model.essence --no-run-solver --rule-trace trace.txt
cat trace.txt
rm trace.txt
