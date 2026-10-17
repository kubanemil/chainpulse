#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5433/postgres"

help() {
  cat <<'USAGE'
Usage: ./do <task> [args]

  test       unit tests
  itest      integration tests against Postgres (Docker) and anvil
  check      rustfmt --check and clippy with warnings as errors
  fmt        format the code
  run        start Postgres and run the indexer against chainpulse.toml
  dev        run the whole stack in Docker Compose
  contracts  rebuild the test token artifact with forge
  help       show this list
USAGE
}

itest() {
  command -v anvil >/dev/null || { echo "anvil not found; install Foundry: https://getfoundry.sh" >&2; exit 1; }
  docker compose -f docker-compose.test.yml up -d --wait
  trap 'docker compose -f docker-compose.test.yml down' EXIT
  DATABASE_URL="$TEST_DATABASE_URL" cargo test --workspace --test '*' -- --ignored "$@"
}

run() {
  if [[ -f .env ]]; then set -a; source .env; set +a; fi
  docker compose up -d --wait postgres
  DATABASE_URL="${DATABASE_URL:-postgres://chainpulse:chainpulse@localhost:5432/chainpulse}" \
    cargo run -p chainpulse
}

contracts() {
  local out
  out="$(mktemp -d)"
  forge build --root "$out" --contracts chainpulse/tests/contracts --out "$out/out" --cache-path "$out/cache" --use 0.8.30
  jq -c '{abi: .abi, bytecode: {object: .bytecode.object}}' "$out/out/TestToken.sol/TestToken.json" \
    > chainpulse/tests/contracts/TestToken.json
  rm -rf "$out"
}

task="${1:-help}"
[[ $# -gt 0 ]] && shift
case "$task" in
  test) cargo test --workspace "$@" ;;
  itest) itest "$@" ;;
  check) cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings ;;
  fmt) cargo fmt --all ;;
  run) run ;;
  dev) docker compose up --build "$@" ;;
  contracts) contracts ;;
  help) help ;;
  *) echo "unknown task: $task" >&2; help >&2; exit 1 ;;
esac
