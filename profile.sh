#!/usr/bin/env bash

# Exit when any command fails
set -e

cargo build --examples --profile=release-with-debug
rm -f plots/data.dat 
rm -f plots/data-sorted.dat 
rm -rf profile-tmp
mkdir profile-tmp

for i in {2..6}; do
    n=$((10**$i))

    for x in {4..1}; do
        nodes=$(($n / $x))
        echo "Profiling with ${nodes} nodes."
        echo -en "\n${nodes} " >> data.dat
        echo -en "\n${nodes} " >> data-sorted.dat
        valgrind --tool=dhat --dhat-out-file=profile-tmp/dhat.out.n-${nodes} ./target/release-with-debug/examples/calculate-root ${nodes} 2>&1 \
            | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$1' | tr -d ',' | tr '\n' ' ' >> data.dat
        valgrind --tool=dhat --dhat-out-file=profile-tmp/dhat-sorted.out.n-${nodes} ./target/release-with-debug/examples/calculate-root-sorted ${nodes} 2>&1 \
            | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$1' | tr -d ',' | tr '\n' ' ' >> data-sorted.dat
    done
done

mv data.dat plots/
mv data-sorted.dat plots/
cd plots
gnuplot plot-profile.plt
