#!/bin/bash
LOG="/opt/polytopia-bot/usage.log"
TODAY=$(date -u +%Y-%m-%d)

if [ ! -f "$LOG" ]; then
    echo "No usage data yet."
    exit 0
fi

echo "Total commands: $(wc -l < "$LOG")"
echo "Today ($TODAY): $(grep -c "$TODAY" "$LOG")"

echo ""
echo "By server:"
grep -oP '"server":"[^"]*"' "$LOG" | cut -d'"' -f4 | sort | uniq -c | sort -rn | while read count name; do
    printf "  %4d  %s\n" "$count" "${name:-(unknown)}"
done

echo ""
echo "By command:"
grep -oP '"command":"[^"]*"' "$LOG" | cut -d'"' -f4 | sort | uniq -c | sort -rn | while read count name; do
    printf "  %4d  /%s\n" "$count" "$name"
done

echo ""
echo "By user:"
grep -oP '"user":"[^"]*"' "$LOG" | cut -d'"' -f4 | sort | uniq -c | sort -rn | while read count name; do
    printf "  %4d  %s\n" "$count" "$name"
done

echo ""
echo "Last 10:"
tail -10 "$LOG" | while IFS= read -r line; do
    time=$(echo "$line" | grep -oP '"time":"[^"]*"' | cut -d'"' -f4)
    server=$(echo "$line" | grep -oP '"server":"[^"]*"' | cut -d'"' -f4)
    user=$(echo "$line" | grep -oP '"user":"[^"]*"' | cut -d'"' -f4)
    cmd=$(echo "$line" | grep -oP '"command":"[^"]*"' | cut -d'"' -f4)
    printf "  %s  %-20s  %-15s  /%s\n" "$time" "${server:-(unknown)}" "$user" "$cmd"
done
