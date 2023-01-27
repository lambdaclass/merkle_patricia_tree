set autoscale                        # scale axes automatically
set key left top
set xtic auto                          # set xtics automatically
set ytic auto                          # set ytics automatically
set title "Patricia Merkle Tree Root Hash Memory Usage"
set xlabel "Nodes Inserted"
set ylabel "Memory Usage"
set term svg
set format y '%.0s%cB'
set output "profile.svg"
plot "data.dat" using 1:2 title 'Total' with linespoints, \
    "data.dat" using 1:3 title 'At t-gmax' with linespoints, \
    "data.dat" using 1:5 title 'Reads' with linespoints, \
    "data.dat" using 1:6 title 'Writes' with linespoints

set logscale xy
set output "profile-logscale.svg"
plot "data.dat" using 1:2 title 'Total' with linespoints, \
    "data.dat" using 1:3 title 'At t-gmax' with linespoints, \
    "data.dat" using 1:5 title 'Reads' with linespoints, \
    "data.dat" using 1:6 title 'Writes' with linespoints
