#!/bin/sh
# Rendered by Statecraft from profile github-actions-rust revision {{sc:profile.revision}}.
# A managed file: `statecraft doctor` names an edit to it. Installs the exact
# spec-spine release this repository pins into .bin/spec-spine, and refuses a
# range or an absent pin rather than resolving one.
set -eu

# The family exit contract (revision 7): 0 ok, 2 refused (no exact pin, a
# precondition the operator supplies), 4 failed (the install or its version
# read broke). Every deliberate non-zero exit goes through `leave`; a command
# `set -e` stops on is reported as 4 by the EXIT trap, whatever its own code.
leave() {
  trap - EXIT
  exit "$1"
}
trap 'rc=$?; if [ "$rc" -ne 0 ]; then echo "install-spec-spine.sh: a command failed (exit $rc); reported as failed (4)" >&2; exit 4; fi' EXIT

pin=$(awk '
  /^[[:space:]]*\[/ { section = $0; gsub(/[[:space:]]/, "", section); next }
  section == "[meta]" && /^[[:space:]]*required_version[[:space:]]*=/ { print; exit }
' spec-spine.toml 2>/dev/null | sed 's/^[^=]*=[[:space:]]*"\(.*\)"[[:space:]]*$/\1/')

case "$pin" in
  =[0-9]*.[0-9]*.[0-9]*) version="${pin#=}" ;;
  "")
    echo "install-spec-spine.sh: spec-spine.toml [meta] carries no required_version; this profile requires an exact pin (=X.Y.Z)" >&2
    leave 2 ;;
  *)
    echo "install-spec-spine.sh: required_version \"$pin\" is not an exact pin (=X.Y.Z); this profile refuses a range" >&2
    leave 2 ;;
esac

bin=.bin/spec-spine
if [ -x "$bin" ] && [ "$("$bin" --version 2>/dev/null)" = "spec-spine $version" ]; then
  echo "spec-spine $version is already installed at $bin"
  exit 0
fi
if ! command -v cargo > /dev/null 2>&1; then
  echo "install-spec-spine.sh: cargo is not on PATH, so spec-spine $version cannot be installed" >&2
  leave 2
fi
# Revision 13: cargo installs into a root's bin/, so it installs into a
# scratch root and the one executable is moved into .bin/. Nothing else of
# cargo's install record is left in the repository.
scratch=$(mktemp -d)
if ! cargo install spec-spine-cli --version "=$version" --locked --root "$scratch"; then
  rm -rf "$scratch"
  echo "install-spec-spine.sh: cargo install of spec-spine $version failed" >&2
  leave 4
fi
mkdir -p .bin
cp "$scratch/bin/spec-spine" "$bin.partial"
chmod 755 "$bin.partial"
mv -f "$bin.partial" "$bin"
rm -rf "$scratch"
if ! "$bin" --version; then
  echo "install-spec-spine.sh: the installed $bin does not answer --version" >&2
  leave 4
fi
