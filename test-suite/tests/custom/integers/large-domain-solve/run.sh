set -e
for channelling in no yes uniform; do
    conjure-oxide --solver sat --channelling "$channelling" solve model.essence --number-of-solutions=all
done
