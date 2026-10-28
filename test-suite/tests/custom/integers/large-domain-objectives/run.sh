set -e
for encoding in rustsat-generalized-totalizer rustsat-binary-adder pindakaas-bdd rustsat-dynamic-poly-watchdog pindakaas-swc; do
    for direction in minimising maximising; do
        conjure-oxide --solver sat --sat-encoding-pb "$encoding" solve "$direction.essence"
    done
done
