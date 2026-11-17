conjure-oxide pretty model.essence --output-format="ast-json"

echo ""
echo ""
echo ""

conjure-oxide-debug solve model.essence --no-run-solver --rule-trace trace.txt
cat trace.txt
rm trace.txt

echo ""
echo ""
echo ""

# Both parsers must build a set difference here, and the model must solve.
conjure-oxide solve model.essence
conjure-oxide solve model.essence
