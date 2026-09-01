conjure-oxide --log-file conjure_oxide_test.data --log-format json solve --parser=via-conjure model.eprime --number-of-solutions=all

[ -f "./conjure_oxide_test.data" ] && echo "./conjure_oxide_test.data found" || echo "./conjure_oxide_test.data is missing"

[ -s "./conjure_oxide_test.data" ] && echo "./conjure_oxide_test.data has been written" || echo "./conjure_oxide_test.data is empty"

rm conjure_oxide_test.data
