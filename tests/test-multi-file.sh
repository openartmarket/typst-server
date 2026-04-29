#!/usr/bin/env bash
# Smoke test: verify that an additional .typ file uploaded as a multipart part
# can be imported by the main template via `#import "<filename>": …`.
#
# Usage: tests/test-multi-file.sh
#
# Builds the binary if needed, starts the server on a free-ish port, posts a
# main + lib pair, and checks that the response is a valid PDF.

set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${PORT:-3019}"
TOKEN="${TYPST_SERVER_TOKEN:-test-token}"
BIN="${ROOT}/target/release/typst-server"

if [[ ! -x "${BIN}" ]]; then
    echo "Building typst-server..."
    (cd "${ROOT}" && cargo build --release)
fi

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

cat > "${WORK}/main.typ" <<'EOF'
#import "lib.typ": greet

= Multi-file smoke test
#greet("typst-server")
EOF

cat > "${WORK}/lib.typ" <<'EOF'
#let greet(name) = [Hi, #name!]
EOF

echo '{}' > "${WORK}/data.json"

PORT="${PORT}" TYPST_SERVER_TOKEN="${TOKEN}" "${BIN}" >"${WORK}/server.log" 2>&1 &
SERVER_PID=$!
cleanup() {
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
    rm -rf "${WORK}"
}
trap cleanup EXIT

# Wait for the server to bind.
for _ in $(seq 1 30); do
    if curl -s -o /dev/null --max-time 1 "http://localhost:${PORT}/" 2>/dev/null; then
        break
    fi
    sleep 0.2
done

STATUS=$(curl -sS -X POST "http://localhost:${PORT}/" -u ":${TOKEN}" \
    -F "template=@${WORK}/main.typ" \
    -F "file=@${WORK}/lib.typ" \
    -F "data=@${WORK}/data.json" \
    -o "${WORK}/out.pdf" -w "%{http_code}")

if [[ "${STATUS}" != "200" ]]; then
    echo "FAIL: HTTP ${STATUS}"
    cat "${WORK}/out.pdf"
    exit 1
fi

if ! head -c 5 "${WORK}/out.pdf" | grep -q "%PDF-"; then
    echo "FAIL: response is not a PDF"
    file "${WORK}/out.pdf"
    exit 1
fi

echo "PASS: multi-file import produces a valid PDF ($(stat -f%z "${WORK}/out.pdf" 2>/dev/null || stat -c%s "${WORK}/out.pdf") bytes)"
exit 0
