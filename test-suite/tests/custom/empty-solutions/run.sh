set -eu

conjure-oxide solve satisfiable.essence --output-format json
conjure-oxide solve unsatisfiable.essence --output-format json
conjure-oxide test-solve satisfiable.essence
conjure-oxide test-solve unsatisfiable.essence
