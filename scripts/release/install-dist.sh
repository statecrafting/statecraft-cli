#!/bin/sh
# Spec 057 section 3.2: cargo-dist at an exact version. Downloads the
# cargo-dist release archive for <host> at $DIST_VERSION, refuses it unless
# its SHA-256 equals the digest the release workflow pins for that host
# ($DIST_SHA256_<HOST>, the triple upper-cased with `-` as `_`), and extracts
# its `dist` binary into <bin-dir>. Nothing is resolved at a floating version
# and nothing is built from source.
#
# Usage: install-dist.sh <host-triple> <bin-dir>
set -eu

host=${1:?usage: install-dist.sh <host-triple> <bin-dir>}
bindir=${2:?usage: install-dist.sh <host-triple> <bin-dir>}
: "${DIST_VERSION:?DIST_VERSION is not set}"

# The host is one of the four section 3.2 names, each read from its own
# variable by name; any other argument refuses before anything is read or
# fetched, and nothing from the argument is evaluated.
case "$host" in
  aarch64-apple-darwin) var=DIST_SHA256_AARCH64_APPLE_DARWIN want=${DIST_SHA256_AARCH64_APPLE_DARWIN:-} ;;
  x86_64-apple-darwin) var=DIST_SHA256_X86_64_APPLE_DARWIN want=${DIST_SHA256_X86_64_APPLE_DARWIN:-} ;;
  aarch64-unknown-linux-gnu) var=DIST_SHA256_AARCH64_UNKNOWN_LINUX_GNU want=${DIST_SHA256_AARCH64_UNKNOWN_LINUX_GNU:-} ;;
  x86_64-unknown-linux-gnu) var=DIST_SHA256_X86_64_UNKNOWN_LINUX_GNU want=${DIST_SHA256_X86_64_UNKNOWN_LINUX_GNU:-} ;;
  *) echo "install-dist: $host is not one of the four release hosts" >&2; exit 1 ;;
esac
printf '%s' "$want" | grep -E -q '^[0-9a-f]{64}$' \
  || { echo "install-dist: no pinned SHA-256 for $host (\$$var)" >&2; exit 1; }

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
archive="cargo-dist-$host.tar.xz"
url="https://github.com/axodotdev/cargo-dist/releases/download/v$DIST_VERSION/$archive"
curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$tmp/$archive" "$url"

if command -v sha256sum >/dev/null 2>&1; then
  got=$(sha256sum "$tmp/$archive" | awk '{ print $1 }')
else
  got=$(shasum -a 256 "$tmp/$archive" | awk '{ print $1 }')
fi
[ "$got" = "$want" ] || { echo "install-dist: $archive is $got, pinned $want" >&2; exit 1; }

tar -xJf "$tmp/$archive" -C "$tmp" "cargo-dist-$host/dist"
mkdir -p "$bindir"
install -m 755 "$tmp/cargo-dist-$host/dist" "$bindir/dist"
reported=$("$bindir/dist" --version)
case "$reported" in
  *" $DIST_VERSION"*) echo "install-dist: $reported ($got)" ;;
  *) echo "install-dist: $archive reports '$reported', not $DIST_VERSION" >&2; exit 1 ;;
esac
