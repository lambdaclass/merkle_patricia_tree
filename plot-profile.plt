set autoscale                        # scale axes automatically
unset log                              # remove any log-scaling
unset label                            # remove any previous labels
set xtic auto                          # set xtics automatically
set ytic auto                          # set ytics automatically
set title "Force Deflection Data for a Beam and a Column"
set xlabel "Nodes Inserted"
set ylabel "Memory in Bytes"
plot "data.dat" using 1:2 title 'Total' with linespoints, \
    "data.dat" using 1:3 title 'At t-gmax' with linespoints, \
    "data.dat" using 1:4 title 'Reads' with linespoints, \
    "data.dat" using 1:5 title 'Writes' with linespoints