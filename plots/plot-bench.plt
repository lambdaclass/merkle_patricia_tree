set autoscale                        # scale axes automatically
set key bottom right
set xtic auto                          # set xtics automatically
set ytic auto                          # set ytics automatically
set xlabel "Key/values Inserted"
set ylabel "Time in ns"

set terminal svg enhanced background rgb 'white'
set object rectangle from screen 0,0 to screen 1,1 behind fillcolor rgb 'white' fillstyle solid noborder
set logscale xy
set xrange [1000:100000000]

set title "Benchmark Get"
set output "bench-gets.svg"

plot "bench-gi.dat" using 1:2 title 'Lambda Get' with linespoints, \
    "bench-gi-geth.dat" using 1:2 title 'Geth Get' with linespoints, \
    "bench-gi-paprika.dat" using 1:2 title 'Paprika Get' with linespoints, \
    "bench-gi-parity.dat" using 1:2 title 'Parity Get' with linespoints

set title "Benchmark Insert"
set output "bench-inserts.svg"
plot "bench-gi.dat" using 1:3 title 'Lambda Insert' with linespoints, \
    "bench-gi-geth.dat" using 1:3 title 'Geth Insert' with linespoints, \
    "bench-gi-paprika.dat" using 1:3 title 'Paprika Insert' with linespoints, \
    "bench-gi-parity.dat" using 1:3 title 'Parity Insert' with linespoints
