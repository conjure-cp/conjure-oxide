# Set difference reached through an equality rather than a comprehension
# generator, so it has to be lowered at the membership level.
conjure-oxide-debug solve model.essence --no-run-solver --rule-trace trace.txt
cat trace.txt
rm trace.txt

echo ""
echo ""
echo ""

conjure-oxide --parser=via-conjure solve model.essence
conjure-oxide solve model.essence
