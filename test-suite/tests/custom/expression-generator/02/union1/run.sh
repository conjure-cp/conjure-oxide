conjure-oxide --parser=via-conjure pretty model.essence --output-format="ast-json"

echo ""
echo ""
echo ""

conjure-oxide-debug --parser=via-conjure solve model.essence --no-run-solver --rule-trace trace.txt
cat trace.txt
rm trace.txt
