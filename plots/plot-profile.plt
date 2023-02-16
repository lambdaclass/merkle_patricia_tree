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

unset logscale
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

set title "Patricia Merkle Tree Sorted Iter Root Hash Memory Usage"
set output "profile-sorted.svg"
unset logscale
plot "data-sorted.dat" using 1:2 title 'Total Allocated' with linespoints, \
    "data-sorted.dat" using 1:3 title 'Max Usage' with linespoints, \
    "data-sorted.dat" using 1:5 title 'Bytes Read' with linespoints, \
    "data-sorted.dat" using 1:6 title 'Bytes Written' with linespoints

set logscale xy
set output "profile-sorted-logscale.svg"
plot "data-sorted.dat" using 1:2 title 'Total Allocated' with linespoints, \
    "data-sorted.dat" using 1:3 title 'Max Usage' with linespoints, \
    "data-sorted.dat" using 1:5 title 'Bytes Read' with linespoints, \
    "data-sorted.dat" using 1:6 title 'Bytes Written' with linespoints

set title "Patricia Merkle Tree Root Hash Memory Usage"
set output "profile-both.svg"
unset logscale
plot "data.dat" using 1:2 title 'Total Allocated' with linespoints, \
    "data.dat" using 1:3 title 'Max Usage' with linespoints, \
    "data-sorted.dat" using 1:2 title 'Total Allocated Sorted' with linespoints, \
    "data-sorted.dat" using 1:3 title 'Max Usage Sorted' with linespoints

set logscale xy
set output "profile-both-logscale.svg"
plot "data.dat" using 1:2 title 'Total Allocated' with linespoints, \
    "data.dat" using 1:3 title 'Max Usage' with linespoints, \
    "data-sorted.dat" using 1:2 title 'Total Allocated Sorted' with linespoints, \
    "data-sorted.dat" using 1:3 title 'Max Usage Sorted' with linespoints
