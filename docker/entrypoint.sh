#!/bin/sh
set -e

# Start tgtd in background for iSCSI target handling (if tgtd binary exists)
if command -v tgtd >/dev/null 2>&1; then
    echo "[iventoymersed] Starting user-space tgtd daemon..."
    tgtd -f &
    TGTD_PID=$!

    # Wait for tgtd control socket to become responsive
    WAIT_COUNT=0
    until tgtadm --lld iscsi --op show --mode target >/dev/null 2>&1 || [ $WAIT_COUNT -gt 30 ]; do
        sleep 0.2
        WAIT_COUNT=$((WAIT_COUNT + 1))
    done

    if tgtadm --lld iscsi --op show --mode target >/dev/null 2>&1; then
        echo "[iventoymersed] tgtd ready (PID $TGTD_PID, iSCSI Port 3260)."
    else
        echo "[iventoymersed] WARNING: tgtd not responsive after 6s. iSCSI may be degraded."
    fi
else
    echo "[iventoymersed] tgtd not found in environment, running in pure HTTP/NBD mode."
fi

# Run iventoymersed
exec /app/iventoymersed "$@"
