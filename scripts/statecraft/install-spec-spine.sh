#!/bin/sh
# Rendered by Statecraft from profile github-actions-rust revision 4.
# A managed file: `statecraft doctor` names an edit to it. Installs the exact
# spec-spine release this repository pins into .tooling/bin, and refuses a
# range or an absent pin rather than resolving one.
set -eu

pin=$(awk '
  /^[[:space:]]*\[/ { section = $0; gsub(/[[:space:]]/, "", section); next }
  section == "[meta]" && /^[[:space:]]*required_version[[:space:]]*=/ { print; exit }
' spec-spine.toml 2>/dev/null | sed 's/^[^=]*=[[:space:]]*"\(.*\)"[[:space:]]*$/\1/')

case "$pin" in
  =[0-9]*.[0-9]*.[0-9]*) version="${pin#=}" ;;
  "")
    echo "install-spec-spine.sh: spec-spine.toml [meta] carries no required_version; this profile requires an exact pin (=X.Y.Z)" >&2
    exit 2 ;;
  *)
    echo "install-spec-spine.sh: required_version \"$pin\" is not an exact pin (=X.Y.Z); this profile refuses a range" >&2
    exit 2 ;;
esac

bin=.tooling/bin/spec-spine
if [ -x "$bin" ] && [ "$("$bin" --version 2>/dev/null)" = "spec-spine $version" ]; then
  echo "spec-spine $version is already installed at $bin"
  exit 0
fi
cargo install spec-spine-cli --version "=$version" --locked --root .tooling
"$bin" --version
