conjure-oxide solve --parser=tree-sitter model.eprime --number-of-solutions=all

[ -f "./conjure_oxide.log" ] && echo "./conjure_oxide.log has been written" || echo "./conjure_oxide.log is missing"
[ -f "./conjure_oxide_log.json" ] && echo "./conjure_oxide_log.json has been written" || echo "./conjure_oxide_log.json is missing"

