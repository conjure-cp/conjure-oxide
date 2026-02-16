conjure-oxide --log --logfile conjure_oxide.log --logfile-json conjure_oxide_log.json solve model.eprime

[ -f "./conjure_oxide.log" ] && echo "./conjure_oxide.log found" || echo "./conjure_oxide.log is missing"
[ -f "./conjure_oxide_log.json" ] && echo "./conjure_oxide_log.json found" || echo "./conjure_oxide_log.json is missing"

[ -s "./conjure_oxide.log" ] && echo "./conjure_oxide.log has been written" || echo "./conjure_oxide.log is empty"
[ -s "./conjure_oxide_log.json" ] && echo "./conjure_oxide_log.json has been written" || echo "./conjure_oxide_log.json is empty"

rm conjure_oxide_log.json conjure_oxide.log
