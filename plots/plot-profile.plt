set autoscale                        # scale axes automatically
set key left top
set xtic auto                          # set xtics automatically
set ytic auto                          # set ytics automatically
set title "Patricia Merkle Tree Root Hash Memory Usage"
set xlabel "Nodes Inserted"
set ylabel "Memory Usage"
set format y '%.0s%cB'
set xrange [25:1000000]
set terminal svg enhanced background rgb 'white'
set object rectangle from screen 0,0 to screen 1,1 behind fillcolor rgb 'white' fillstyle solid noborder

set output "profile.svg"
plot "data.dat" using 1:2 title 'Total Allocated' with linespoints, \
    "data.dat" using 1:3 title 'Max Usage' with linespoints, \
    "data.dat" using 1:5 title 'Bytes Read' with linespoints, \
    "data.dat" using 1:6 title 'Bytes Written' with linespoints

set logscale xy
set output "profile-logscale.svg"
plot "data.dat" using 1:2 title 'Total Allocated' with linespoints, \
    "data.dat" using 1:3 title 'Max Usage' with linespoints, \
    "data.dat" using 1:5 title 'Bytes Read' with linespoints, \
    "data.dat" using 1:6 title 'Bytes Written' with linespoints
