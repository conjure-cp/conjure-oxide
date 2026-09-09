# Unrolling `forAll i : D` over a large index domain has to stay linear in the
# number of constraints it produces. It was once quadratic: N=4000 took ~42s and
# N=8000 never finished. `expected-time` in config.toml tracks the cost, so a
# return to quadratic behaviour shows up as a much larger recorded time.
#
# Counting the constraints rather than printing them keeps the snapshot small,
# and catches a rewrite that silently drops them instead of getting faster.
conjure-oxide solve --no-run-solver model.essence model.param | grep -c '^Ineq'
