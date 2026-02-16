conjure-oxide --log --logfile conjure_oxide_test --logfile-json conjure_oxide_log_test solve model.eprime

[ -f "./conjure_oxide_test.log" ] && echo "./conjure_oxide_test.log found" || echo "./conjure_oxide_test.log is missing"
[ -f "./conjure_oxide_log_test.json" ] && echo "./conjure_oxide_log_test.json found" || echo "./conjure_oxide_log_test.json is missing"

[ -s "./conjure_oxide_test.log" ] && echo "./conjure_oxide_test.log has been written" || echo "./conjure_oxide_test.log is empty"
[ -s "./conjure_oxide_log_test.json" ] && echo "./conjure_oxide_log_test.json has been written" || echo "./conjure_oxide_log_test.json is empty"
