#!/usr/bin/env bash
# Pushes store paths to niks3 (niks3.andyyu.io, read at nix-cache.andyyu.io). Its container
# stops when idle, so the first request can fail while it starts. Without a token (a fork's
# pull request) it pushes nothing.
set -euo pipefail
[ -n "${NIKS3_AUTH_TOKEN:-}" ] || exit 0
token=$(mktemp)
trap 'rm -f "$token"' EXIT
printf '%s' "$NIKS3_AUTH_TOKEN" >"$token"
nix shell --inputs-from . nixpkgs#niks3 -c niks3 push \
  --server-url https://niks3.andyyu.io --auth-token-path "$token" --retry-timeout 2m "$@"
