set autoscale                        # scale axes automatically
set key bottom right
set xtic auto                          # set xtics automatically
set ytic auto                          # set ytics automatically
set xlabel "Key/values Inserted"
set ylabel "Time in ns"
set term svg
set logscale xy
set xrange [1000:1000000]

set title "Benchmark Get"
set output "bench-gets.svg"
plot "bench-gi.dat" using 1:2 title 'Get' with linespoints, \
    "bench-gi-geth.dat" using 1:2 title 'Geth Get' with linespoints, \
    "bench-gi-paprika.dat" using 1:2 title 'Paprika Get' with linespoints, \
    "bench-gi-parity.dat" using 1:2 title 'Parity Get' with linespoints

set title "Benchmark Insert"
set output "bench-inserts.svg"
plot "bench-gi.dat" using 1:3 title 'Insert' with linespoints, \
    "bench-gi-geth.dat" using 1:3 title 'Geth Insert' with linespoints, \
    "bench-gi-paprika.dat" using 1:3 title 'Paprika Insert' with linespoints, \
    "bench-gi-parity.dat" using 1:3 title 'Parity Insert' with linespoints
