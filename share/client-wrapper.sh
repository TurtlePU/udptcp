#!/bin/ash
while true; do
    /udptcp -H $1 -p $2 < share/input.txt
    sleep $3
done
