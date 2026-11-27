#!/system/bin/sh
# Measures over a window of D seconds: CPU (utime+stime) of the app and of its WebView
# renderer process, busiest threads, wlan0 bytes, frames (gfxinfo).
# Usage (#451, see docs/android.md "Performance measurements"):
#   adb push scripts/android-perf.sh /data/local/tmp/ ; adb shell [PKG=<package>] sh /data/local/tmp/android-perf.sh <seconds> [nogfx]
# `nogfx`: does not reset the frame counters (measurement with the screen off).
D=${1:-60}
PKG=${PKG:-dev.seaglass.app}  # another app: PKG=com.aspiro.tidal sh ...
A=$(pidof $PKG)
APPUID=$(stat -c %u /proc/$A)
TAG="u0a$((APPUID - 10000))i"
R=$(dumpsys activity processes 2>/dev/null | grep -o "[0-9]*:com.google.android.webview:sandboxed[^/]*/$TAG" | head -1 | cut -d: -f1)
HZ=100
cpu() { awk '{print $14+$15}' /proc/$1/stat; }
threads() { for t in /proc/$1/task/*; do awk -v p=$2 '{c=$0; sub(/^[0-9]+ \(/,"",c); n=c; sub(/\).*/,"",n); sub(/^[^)]*\) /,"",c); split(c,f," "); print p":"$1, f[12]+f[13], n}' $t/stat 2>/dev/null; done; }
net() { awk '/^ *wlan0:/ {sub(/.*wlan0:/,""); split($0,f," "); s+=f[1]} END {printf "%.0f", s}' /proc/net/dev; }
[ "$2" != "nogfx" ] && dumpsys gfxinfo $PKG reset >/dev/null
threads $A app > /data/local/tmp/th0; [ -n "$R" ] && threads $R rnd >> /data/local/tmp/th0
a0=$(cpu $A); r0=$( [ -n "$R" ] && cpu $R || echo 0); n0=$(net); t0=$(date +%s)
sleep $D
a1=$(cpu $A); r1=$( [ -n "$R" ] && cpu $R || echo 0); n1=$(net); t1=$(date +%s)
threads $A app > /data/local/tmp/th1; [ -n "$R" ] && threads $R rnd >> /data/local/tmp/th1
S=$((t1 - t0))
echo "window ${S}s app=$A renderer=$R"
awk -v a=$((a1-a0)) -v r=$((r1-r0)) -v s=$S -v hz=$HZ -v n0=$n0 -v n1=$n1 'BEGIN{printf "CPU app %.1f %% of one core, renderer %.1f %%, total %.1f %% ; wlan0 received %.0f KB\n", a/hz/s*100, r/hz/s*100, (a+r)/hz/s*100, (n1-n0)/1024}'
awk 'NR==FNR {b[$1]=$2; next} {d=$2-b[$1]; if (d>0) {$2=""; printf "%6.1f %% %s\n", d/'$HZ'/'$S'*100, $0}}' /data/local/tmp/th0 /data/local/tmp/th1 | sort -rn | head -8
echo "memory (KB): app $(awk '/VmRSS/{print $2}' /proc/$A/status) RSS, renderer $( [ -n "$R" ] && awk '/VmRSS/{print $2}' /proc/$R/status) RSS"
dumpsys meminfo $A | grep -E "TOTAL PSS|^ *Java Heap:|^ *Native Heap:|^ *Graphics:"
if [ "$2" != "nogfx" ]; then dumpsys gfxinfo $PKG | grep -E "Total frames rendered|Janky frames|50th|90th|95th|99th|Number Missed Vsync|Number Slow UI|Number Frame deadline" | grep -v legacy | head -10; fi
