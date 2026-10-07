conjure-oxide --log-file conjure_oxide.log solve --parser=via-conjure model.eprime --number-of-solutions=all

[ -f "./conjure_oxide.log" ] && echo "./conjure_oxide.log found" || echo "./conjure_oxide.log is missing"
[ -s "./conjure_oxide.log" ] && echo "./conjure_oxide.log has been written" || echo "./conjure_oxide.log is empty"

rm conjure_oxide.log
