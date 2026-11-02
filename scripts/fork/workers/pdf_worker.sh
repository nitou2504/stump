#!/bin/bash
# Entrypoint of the Stump PDF worker container (docker/Dockerfile.pdf-worker).
#
# Waits for Stump to queue a PDF in $STUMP_PDF_QUEUE/queue/ and runs `pdf_runner.py drain` only
# then (at nice 19 and idle I/O priority); drain works through the queue and exits. While it
# waits, the container holds this shell and one inotifywait (a few MB). Every
# $PDF_WORKER_HEARTBEAT seconds it refreshes $STUMP_PDF_QUEUE/worker.json, which Stump shows as
# the worker's state in the library's post-processing settings.
#
# inotifywait runs in monitor mode for the life of the container, so a request written while
# the shell is busy (draining, or checking the folder) is still an event in its pipe: none is
# missed. PDF_WORKER_POLL=<seconds> checks the folder on a timer instead (for file systems that
# do not deliver inotify events, e.g. some network shares).
set -u

QUEUE=${STUMP_PDF_QUEUE:-/config/workers/pdf}
INTERVAL=${PDF_WORKER_HEARTBEAT:-300}
POLL=${PDF_WORKER_POLL:-}
RUNNER=${PDF_RUNNER:-/opt/stump-pdf-worker/pdf_runner.py}
PYTHON=${PDF_WORKER_PYTHON:-python3}
export STUMP_PDF_QUEUE="$QUEUE" PDF_WORKER_HEARTBEAT="$INTERVAL"
PDF_WORKER_STARTED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
export PDF_WORKER_STARTED_AT
last_beat=0
watcher=

log() { echo "$(date '+%Y-%m-%d %H:%M:%S') $*"; }

heartbeat() {
	local now
	now=$(date +%s)
	printf '{"version":1,"worker":"%s","state":"idle","current":null,"pid":%s,"started_at":"%s","updated_at":"%s","updated_unix":%s,"interval":%s}\n' \
		"${PDF_WORKER_VERSION:-dev}" "$$" "$PDF_WORKER_STARTED_AT" \
		"$(date -u -d "@$now" +%Y-%m-%dT%H:%M:%SZ)" "$now" "$INTERVAL" >"$QUEUE/worker.json.tmp" &&
		mv -f "$QUEUE/worker.json.tmp" "$QUEUE/worker.json"
	last_beat=$now
}

queued() {
	local f
	for f in "$QUEUE"/queue/*.json; do
		[ -e "$f" ] && return 0
	done
	return 1
}

drain() {
	nice -n 19 ionice -c 3 "$PYTHON" "$RUNNER" drain
}

# inotifywait -m on fd 3: one line per event in queue/ (a new or renamed request)
start_watcher() {
	exec 3< <(exec inotifywait -m -q -e create -e moved_to -e close_write --format '%f' "$QUEUE/queue")
	watcher=$!
}

# returns when an event arrived or after $1 seconds; restarts inotifywait if it died
wait_event() {
	local name rc
	read -r -t "$1" name <&3
	rc=$?
	if [ "$rc" -eq 0 ]; then
		# a burst (Stump writes <id>.json.tmp, then renames it): take what is already there
		while read -r -t 1 name <&3; do :; done
	elif [ "$rc" -le 128 ]; then
		log "inotifywait stopped; restarting it in 30 s"
		exec 3<&-
		sleep 30
		start_watcher
	fi
}

trap 'log "stopping"; [ -n "$watcher" ] && kill "$watcher" 2>/dev/null; exit 0' TERM INT

if ! mkdir -p "$QUEUE/queue" "$QUEUE/running" "$QUEUE/done" "$QUEUE/archive" 2>/dev/null; then
	log "cannot write to $QUEUE: mount Stump's <config>/workers/pdf there, writable by $(id -u):$(id -g)"
	exit 1
fi
if [ -n "$POLL" ]; then how="poll every ${POLL}s"; else how=inotify; start_watcher; fi
log "PDF worker ${PDF_WORKER_VERSION:-dev} watching $QUEUE/queue ($how, heartbeat every ${INTERVAL}s)"
heartbeat
# requests queued (or left running) while the worker was down
drain
heartbeat

while :; do
	if [ -n "$POLL" ]; then
		sleep "$POLL" &
		wait $!
	else
		wait_event "$INTERVAL"
	fi
	if queued; then
		drain
		heartbeat
	elif [ $(($(date +%s) - last_beat)) -ge "$INTERVAL" ]; then
		heartbeat
	fi
done
