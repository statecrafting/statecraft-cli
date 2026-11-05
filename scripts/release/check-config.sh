#!/bin/sh
# Spec 057: the release configuration, checked offline from the committed
# text. It needs no network, no cargo and no cargo-dist, so it runs in the
# corpus gate's world and in the release workflow's admission job before any
# build. It judges two unclaimed authority files (section 2, O-4): the
# `[workspace.metadata.dist]` table of the root Cargo.toml and
# .github/workflows/release.yml.
#
# Usage: check-config.sh [<root>]
#        check-config.sh --self-test
set -eu

TARGETS="aarch64-apple-darwin x86_64-apple-darwin aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu"

# Print the body of one TOML table: the lines after `[name]` up to the next
# table header. Comments and blank lines are dropped.
toml_table() {
  awk -v want="[$2]" '
    /^\[/ { inside = ($0 == want); next }
    inside && !/^[[:space:]]*(#|$)/ { print }
  ' "$1"
}

# The value of `key = value` in a table body on stdin, quotes removed.
toml_value() {
  awk -v key="$1" '
    { line = $0; sub(/^[[:space:]]+/, "", line) }
    index(line, key " ") == 1 || index(line, key "=") == 1 {
      sub(/^[^=]*=[[:space:]]*/, "", line); gsub(/"/, "", line); sub(/[[:space:]]+$/, "", line)
      print line; exit
    }'
}

check() {
  root=$1 failures=0
  fail() { echo "check-config: $1" >&2; failures=$((failures + 1)); }
  manifest="$root/Cargo.toml"
  workflow="$root/.github/workflows/release.yml"
  [ -s "$manifest" ] || { echo "check-config: no $manifest" >&2; return 1; }
  [ -s "$workflow" ] || { echo "check-config: no $workflow" >&2; return 1; }

  # Section 3.5: nothing goes to crates.io.
  [ "$(toml_table "$manifest" workspace.package | toml_value publish)" = false ] \
    || fail "[workspace.package] publish is not false (section 3.5)"

  dist=$(toml_table "$manifest" workspace.metadata.dist)
  [ -n "$dist" ] || fail "the root Cargo.toml has no [workspace.metadata.dist] table"
  version=$(printf '%s\n' "$dist" | toml_value cargo-dist-version)
  printf '%s' "$version" | grep -E -q '^[0-9]+\.[0-9]+\.[0-9]+$' \
    || fail "cargo-dist-version '$version' is not an exact version (section 3.2)"
  [ "$(printf '%s\n' "$dist" | toml_value dist)" = true ] \
    || fail "[workspace.metadata.dist] dist is not true, so publish = false would drop the binary"
  [ "$(printf '%s\n' "$dist" | toml_value checksum)" = sha256 ] \
    || fail "checksum is not sha256 (section 3.2)"
  installers=$(printf '%s\n' "$dist" | toml_value installers)
  for want in shell homebrew; do
    case "$installers" in *"$want"*) ;; *) fail "installers lacks $want (section 3.3)" ;; esac
  done
  for absent in tap publish-jobs; do
    [ -z "$(printf '%s\n' "$dist" | toml_value "$absent")" ] \
      || fail "[workspace.metadata.dist] sets $absent; no tap is named (O-2) and no job publishes"
  done
  # The targets list spans lines; collect every quoted triple in the table.
  listed=$(printf '%s\n' "$dist" | awk '/^targets[[:space:]]*=/ { on = 1 } on { print } on && /\]/ { exit }' \
    | grep -E -o '"[a-z0-9_]+-[a-z0-9_-]+"' | tr -d '"' | sort | tr '\n' ' ')
  # shellcheck disable=SC2086 # TARGETS is a word list.
  expected=$(printf '%s\n' $TARGETS | sort | tr '\n' ' ')
  [ "$listed" = "$expected" ] || fail "dist targets are '$listed', not '$expected' (section 3.2)"
  [ -n "$(toml_table "$manifest" profile.dist | toml_value inherits)" ] \
    || fail "the root Cargo.toml has no [profile.dist], which cargo-dist builds with"

  # O-1: the one binary is statecraft-cli.
  bins=$(cd "$root" && ls crates/*/src/main.rs 2>/dev/null | tr '\n' ' ')
  [ "$bins" = "crates/statecraft-cli/src/main.rs " ] \
    || fail "binary crates are '$bins'; the release distributes only statecraft-cli (O-1)"
  binname=$(toml_table "$root/crates/statecraft-cli/Cargo.toml" '[bin]' | toml_value name)
  [ "$binname" = statecraft-cli ] || fail "the installed binary is '$binname', not statecraft-cli (O-1)"

  # The workflow's cargo-dist pin is the manifest's, with a digest per host.
  wf_version=$(awk '$1 == "DIST_VERSION:" { gsub(/"/, "", $2); print $2; exit }' "$workflow")
  [ "$wf_version" = "$version" ] \
    || fail "the workflow installs cargo-dist $wf_version, the manifest names $version"
  for host in $(awk '/host:/ { for (i = 1; i <= NF; i++) if ($i == "host:") { h = $(i + 1); gsub(/[,}]/, "", h); print h } }' "$workflow" | sort -u); do
    var="DIST_SHA256_$(printf '%s' "$host" | tr 'a-z-' 'A-Z_')"
    grep -E -q "^[[:space:]]+$var: \"[0-9a-f]{64}\"" "$workflow" \
      || fail "the workflow pins no SHA-256 for the cargo-dist archive of $host ($var)"
  done
  for t in $TARGETS; do
    grep -E -q "target: ${t}[ ,]" "$workflow" || fail "the build matrix lacks $t (section 3.2)"
  done

  # Section 3.1: a tag push is the only trigger.
  triggers=$(awk '
    /^on:/ { on = 1; next }
    on && /^[^[:space:]]/ { exit }
    on && /^  [a-z_]+:/ { t = $1; sub(/:$/, "", t); print t }
  ' "$workflow" | sort -u | tr '\n' ' ')
  [ "$triggers" = "push " ] || fail "the workflow triggers on '$triggers', not a tag push only (section 3.1)"
  if awk '/^on:/ { on = 1; next } on && /^[^[:space:]]/ { exit } on && /branches/ { found = 1 } END { exit !found }' "$workflow"; then
    fail "the workflow runs on a branch push (section 3.1)"
  fi

  # Section 3.2 and spec 044: every action by 40-hex commit digest.
  bad_uses=$(grep -E '^[[:space:]-]*uses:' "$workflow" \
    | grep -v -E 'uses:[[:space:]]*\./' \
    | grep -v -E 'uses:[[:space:]]*[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[0-9a-f]{40}([[:space:]]|$)' || true)
  [ -z "$bad_uses" ] || fail "an action is not pinned by commit digest: $(echo "$bad_uses" | head -1 | sed 's/^[[:space:]]*//')"

  # The steps each section and obligation depends on.
  need() { grep -F -q -- "$1" "$workflow" || fail "the workflow lacks $2: $1"; }
  need 'permissions: {}' "default-deny permissions"
  need '.verification.verified' "the signed-tag refusal (section 3.1)"
  need 'git merge-base --is-ancestor' "the on-main refusal (section 3.1)"
  need '(O-3)' "the version refusal (O-3)"
  need 'cargo fetch --locked' "the locked fetch (section 3.2)"
  need 'git diff --exit-code -- Cargo.lock' "the lockfile proof (section 3.2)"
  need 'actions/attest-build-provenance@' "the provenance attestation (section 3.4)"
  need 'anchore/sbom-action@' "the SBOM (section 3.4)"
  need 'format: cyclonedx-json' "a CycloneDX SBOM (section 3.4)"
  need 'zero components' "the empty-SBOM refusal (R-2)"
  need 'scripts/release/install-dist.sh' "the verified cargo-dist install (section 3.2)"
  need 'scripts/release/harden-installer.sh' "the installer hardening (section 3.3)"
  need 'scripts/release/verify-set.sh' "the release-set verification (R-2)"
  need "if: needs.admit.outputs.mode == 'release'" "the dry-run publish skip (V-1)"
  need 'needs: [admit, verify]' "the publish gate on verification (R-2)"
  need '--verify-tag' "the no-new-tag publish (section 3.5)"

  # Section 3.5: nothing to a registry, nothing back to the repository.
  for forbidden in 'cargo publish' 'git push' 'npm publish' 'twine' 'gh-action-pypi-publish' 'homebrew-' 'contents: write'; do
    count=$(grep -F -c -- "$forbidden" "$workflow" || true)
    if [ "$forbidden" = 'contents: write' ]; then
      [ "$count" -le 1 ] || fail "more than one job may write contents"
    else
      [ "$count" -eq 0 ] || fail "the workflow contains '$forbidden' (section 3.5)"
    fi
  done

  if [ "$failures" -ne 0 ]; then
    echo "check-config: $failures check(s) failed" >&2
    return 1
  fi
  echo "check-config: the dist table and the release workflow meet spec 057"
}

self_test() {
  # `check` assigns `root`, so the source tree is held under another name.
  src=$(cd "$(dirname "$0")/../.." && pwd)
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  stfail() { echo "check-config self-test: $1" >&2; exit 1; }

  fresh() {
    rm -rf "$tmp/r"; mkdir -p "$tmp/r/.github/workflows" "$tmp/r/crates/statecraft-cli/src"
    cp "$src/Cargo.toml" "$tmp/r/Cargo.toml"
    cp "$src/.github/workflows/release.yml" "$tmp/r/.github/workflows/release.yml"
    cp "$src/crates/statecraft-cli/Cargo.toml" "$tmp/r/crates/statecraft-cli/Cargo.toml"
    : > "$tmp/r/crates/statecraft-cli/src/main.rs"
  }
  edit() { # edit <file> <sed expression>
    sed "$2" "$tmp/r/$1" > "$tmp/edit" && mv "$tmp/edit" "$tmp/r/$1"
  }
  refused() { # refused <what>
    if check "$tmp/r" >/dev/null 2>&1; then stfail "accepted $1"; fi
  }

  fresh; check "$tmp/r" >/dev/null 2>&1 || { check "$tmp/r"; stfail "the committed configuration was refused"; }

  fresh; edit .github/workflows/release.yml 's|actions/checkout@[0-9a-f]\{40\}|actions/checkout@v7|'; refused "an action pinned by tag"
  fresh; edit .github/workflows/release.yml 's|^on:$|on:\
  workflow_dispatch:|'; refused "a dispatch trigger"
  fresh; edit .github/workflows/release.yml 's|^    tags:$|    branches: [main]\
    tags:|'; refused "a branch trigger"
  fresh; edit .github/workflows/release.yml 's|DIST_VERSION: "[0-9.]*"|DIST_VERSION: "0.0.1"|'; refused "a cargo-dist version that differs from the manifest"
  fresh; edit .github/workflows/release.yml '/DIST_SHA256_AARCH64_APPLE_DARWIN/d'; refused "a host archive without a pinned digest"
  fresh; edit .github/workflows/release.yml "s|if: needs.admit.outputs.mode == 'release'||"; refused "a publish job a dry run reaches"
  fresh; edit .github/workflows/release.yml 's|zero components|no components|g'; refused "no empty-SBOM refusal"
  fresh; printf '      - run: cargo publish -p statecraft-cli\n' >> "$tmp/r/.github/workflows/release.yml"; refused "a crates.io publish"
  fresh; edit Cargo.toml '/"x86_64-apple-darwin",/d'; refused "a missing target"
  fresh; edit Cargo.toml 's|^installers = .*|installers = ["shell"]|'; refused "no Homebrew formula"
  fresh; edit Cargo.toml 's|^allow-dirty = .*|&\
tap = "statecrafting/homebrew-tap"|'; refused "a tap the owner has not named"
  fresh; edit Cargo.toml 's|^cargo-dist-version = .*|cargo-dist-version = "^0.32"|'; refused "a floating cargo-dist version"
  fresh; edit Cargo.toml 's|^publish = false|publish = true|'; refused "publish = true"
  fresh; edit crates/statecraft-cli/Cargo.toml 's|^name = "statecraft-cli"$|name = "statecraft"|'; refused "a renamed binary"

  echo "check-config self-test: ok"
}

case "${1:-}" in
  --self-test) self_test ;;
  -*) echo "usage: check-config.sh [<root>] | --self-test" >&2; exit 2 ;;
  "") check "$(cd "$(dirname "$0")/../.." && pwd)" ;;
  *) check "$1" ;;
esac
