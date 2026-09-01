conjure-oxide --log-file conjure_oxide.log --log-detail attempts solve --parser=via-conjure model.eprime --number-of-solutions=all

[ -f "./conjure_oxide.log" ] && echo "./conjure_oxide.log found" || echo "./conjure_oxide.log is missing"
[ -s "./conjure_oxide.log" ] && echo "./conjure_oxide.log has been written" || echo "./conjure_oxide.log is empty"
grep -q "attempted rule" conjure_oxide.log && echo "./conjure_oxide.log contains rule attempts" || echo "./conjure_oxide.log is missing rule attempts"

rm conjure_oxide.log
