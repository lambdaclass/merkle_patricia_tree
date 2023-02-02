#!/usr/bin/env bash

# Exit when any command fails
set -e

cargo build --examples --profile=release-with-debug
rm -f plots/data.dat 
rm -rf profile-tmp
mkdir profile-tmp

for i in {1000..1000000..5000}; do
    echo "Profiling with ${i} nodes."
    echo -en "${i} " >> data.dat
    valgrind --tool=dhat --dhat-out-file=profile-tmp/dhat.out.n-${i} ./target/release-with-debug/examples/calculate-root ${i} 2>&1 \
        | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$1' | tr -d ',' | tr '\n' ' ' >> data.dat
done

mv data.dat plots/
cd plots
gnuplot plot-profile.plt
