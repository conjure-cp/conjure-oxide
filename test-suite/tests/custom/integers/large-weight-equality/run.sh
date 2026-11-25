set -e
conjure-oxide --solver sat --sat-encoding-pb rustsat-binary-adder solve model.essence --number-of-solutions=all
