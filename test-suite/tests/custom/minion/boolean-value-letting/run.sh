conjure-oxide --solver minion solve model.essence --number-of-solutions=1 --rule-trace-aggregates aggregates.txt
cat aggregates.txt
rm aggregates.txt

conjure-oxide --solver sat solve model.essence --number-of-solutions=1 --rule-trace-aggregates aggregates.txt
cat aggregates.txt
rm aggregates.txt

conjure-oxide --solver z3 solve model.essence --number-of-solutions=1 --rule-trace-aggregates aggregates.txt
cat aggregates.txt
rm aggregates.txt
