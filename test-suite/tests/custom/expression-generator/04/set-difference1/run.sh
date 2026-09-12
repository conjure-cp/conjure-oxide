conjure-oxide --parser=via-conjure pretty model.essence --output-format="ast-json"

echo ""
echo ""
echo ""

conjure-oxide-debug --parser=via-conjure solve model.essence --no-run-solver --rule-trace trace.txt
cat trace.txt
rm trace.txt

echo ""
echo ""
echo ""

# Both parsers must build a set difference here, and the model must solve.
conjure-oxide --parser=via-conjure solve model.essence
conjure-oxide solve model.essence
