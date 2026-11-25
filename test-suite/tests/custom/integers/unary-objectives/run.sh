set -e
for direction in minimising maximising; do
    conjure-oxide --solver sat --sat-encoding-pb rustsat-binary-adder solve "$direction.essence"
done
