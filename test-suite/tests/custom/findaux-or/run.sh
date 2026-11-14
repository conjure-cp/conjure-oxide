conjure-oxide --solver minion solve model.essence --number-of-solutions=all | grep 'letting x'
conjure-oxide --solver sat solve model.essence --number-of-solutions=all | grep 'letting x'
conjure-oxide --solver z3 solve model.essence --number-of-solutions=all | grep 'letting x'
