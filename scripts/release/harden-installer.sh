#!/bin/sh
# Spec 057 section 3.3: the shell installer checks the archive's SHA-256 and
# installs with `install -m 755`, never `cp`.
#
# cargo-dist's generated installer (pinned in the root Cargo.toml,
# `cargo-dist-version`) places the binary with `mv` and `chmod +x`, and skips
# the checksum, with a message and success, when `sha256sum` is absent (stock
# macOS ships `shasum`), when the installer carries no checksum, or when the
# style is unknown. This script rewrites exactly those lines in the generated
# file, in place:
#
#   1. the binary is written into the install directory's temporary
#      directory by `install -m 755` (the final step stays cargo-dist's
#      same-filesystem `mv`, a rename that gives the path a new inode, which is
#      the property the `cp` prohibition protects);
#   2. SHA-256 falls back to `shasum -a 256`, and the installer refuses rather
#      than skips when neither command exists;
#   3. a missing checksum, or an unknown style, refuses the install.
#
# Every rewrite must match exactly once. A different count means the template
# changed under a new cargo-dist version, and the script fails closed rather
# than ship an installer it has not hardened.
#
# Usage: harden-installer.sh <installer.sh>
#        harden-installer.sh --self-test
set -eu

harden() {
  file=$1
  [ -s "$file" ] || { echo "harden-installer: no installer at $file" >&2; return 1; }
  out="$file.hardened.$$"
  # awk compares each whole line with its leading whitespace removed, so the
  # match is literal (no regular expression over shell text) and indentation is
  # kept. Each rule counts its hits; END fails unless every count is 1.
  awk '
    function lead(s) { match(s, /^[ \t]*/); return substr(s, 1, RLENGTH) }
    function body(s) { sub(/^[ \t]*/, "", s); return s }
    BEGIN {
      n = 0
      from[++n] = "ensure mv \"$_src_dir/$_bin_name\" \"$_install_temp\""
      to[n]     = "ensure install -m 755 \"$_src_dir/$_bin_name\" \"$_install_temp/$_bin_name\""
      from[++n] = "if ! check_cmd sha256sum; then"
      to[n]     = "if ! check_cmd sha256sum && ! check_cmd shasum; then"
      from[++n] = "say \"skipping sha256 checksum verification (it requires the '"'"'sha256sum'"'"' command)\""
      to[n]     = "err \"refusing to install: neither sha256sum nor shasum is available to verify $_file\""
      from[++n] = "_calculated_checksum=\"$(sha256sum -b \"$_file\" | awk '"'"'{printf $1}'"'"')\""
      to[n]     = "if check_cmd sha256sum; then _calculated_checksum=\"$(sha256sum -b \"$_file\" | awk '"'"'{printf $1}'"'"')\"; else _calculated_checksum=\"$(shasum -a 256 -b \"$_file\" | awk '"'"'{printf $1}'"'"')\"; fi"
      from[++n] = "if [ -z \"$_checksum_value\" ]; then"
      to[n]     = "if [ -z \"$_checksum_value\" ]; then err \"refusing to install: the installer carries no checksum for $_file\"; fi; if false; then"
      from[++n] = "say \"no checksums to verify\" 1>&2"
      to[n]     = "err \"refusing to install: no checksum to verify $_file\""
      from[++n] = "say \"skipping unknown checksum style: $_checksum_style\""
      to[n]     = "err \"refusing to install: unknown checksum style $_checksum_style\""
    }
    {
      b = body($0)
      for (i = 1; i <= n; i++) {
        if (b == from[i]) { hit[i]++; print lead($0) to[i]; next }
      }
      print
    }
    END {
      bad = 0
      for (i = 1; i <= n; i++) {
        if (hit[i] != 1) {
          printf "harden-installer: expected one line, found %d: %s\n", hit[i] + 0, from[i] > "/dev/stderr"
          bad = 1
        }
      }
      exit bad
    }
  ' "$file" > "$out" || { rm -f "$out"; return 1; }

  # The result must carry the hardened forms and none of the replaced ones.
  for want in 'ensure install -m 755 ' 'shasum -a 256' 'refusing to install: the installer carries no checksum'; do
    grep -F -q "$want" "$out" || { echo "harden-installer: hardened installer lacks: $want" >&2; rm -f "$out"; return 1; }
  done
  if grep -E -q '(^|[;&|[:space:]])(ensure[[:space:]]+)?cp[[:space:]].*\$_bin_name' "$out"; then
    echo "harden-installer: the installer copies the binary with cp" >&2; rm -f "$out"; return 1
  fi
  if grep -F -q 'skipping sha256 checksum verification' "$out"; then
    echo "harden-installer: the installer still skips SHA-256 verification" >&2; rm -f "$out"; return 1
  fi
  sh -n "$out" || { echo "harden-installer: the hardened installer does not parse" >&2; rm -f "$out"; return 1; }
  chmod 755 "$out"
  mv "$out" "$file"
}

# A fixture holding the lines this script rewrites, as cargo-dist 0.32.0's
# templates/installer/installer.sh.j2 renders them, around enough of their
# surroundings to run. It is not the installer; the release workflow runs this
# script on the real one and fails if any rewrite does not match.
fixture() {
  cat <<'FIXTURE'
#!/bin/sh
say() { echo "$1"; }
err() { say "ERROR: $1" >&2; exit 1; }
check_cmd() { command -v "$1" > /dev/null 2>&1; }
ensure() { if ! "$@"; then err "command failed: $*"; fi; }
place() {
    local _src_dir="$1"
    local _install_temp="$2"
    local _bin_name="$3"
        ensure mv "$_src_dir/$_bin_name" "$_install_temp"
        # unzip seems to need this chmod
        ensure chmod +x "$_install_temp/$_bin_name"
}
fetch() {
    local _file="$1"
    local _checksum_style="$2"
    local _checksum_value="$3"
        if [ -n "${_checksum_style:-}" ]; then
            verify_checksum "$_file" "$_checksum_style" "$_checksum_value"
        else
            say "no checksums to verify" 1>&2
        fi
}
verify_checksum() {
    local _file="$1"
    local _checksum_style="$2"
    local _checksum_value="$3"
    local _calculated_checksum

    if [ -z "$_checksum_value" ]; then
        return 0
    fi
    case "$_checksum_style" in
        sha256)
            if ! check_cmd sha256sum; then
                say "skipping sha256 checksum verification (it requires the 'sha256sum' command)"
                return 0
            fi
            _calculated_checksum="$(sha256sum -b "$_file" | awk '{printf $1}')"
            ;;
        *)
            say "skipping unknown checksum style: $_checksum_style"
            return 0
            ;;
    esac

    if [ "$_calculated_checksum" != "$_checksum_value" ]; then
        err "checksum mismatch
            want: $_checksum_value
            got:  $_calculated_checksum"
    fi
}
"$@"
FIXTURE
}

self_test() {
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  fail() { echo "harden-installer self-test: $1" >&2; exit 1; }

  fixture > "$tmp/installer.sh"
  harden "$tmp/installer.sh" || fail "the fixture did not harden"
  inst="$tmp/installer.sh"

  # Placement goes through install -m 755 and yields an executable file.
  mkdir -p "$tmp/src" "$tmp/dest"
  printf 'bin' > "$tmp/src/tool"
  chmod 600 "$tmp/src/tool"
  sh "$inst" place "$tmp/src" "$tmp/dest" tool >/dev/null || fail "place failed"
  [ -x "$tmp/dest/tool" ] || fail "placed binary is not executable"
  mode=$(ls -l "$tmp/dest/tool" | cut -c1-10)
  [ "$mode" = "-rwxr-xr-x" ] || fail "placed binary mode is $mode, not 755"

  # A matching SHA-256 passes, a wrong one refuses, a missing one refuses.
  printf 'archive' > "$tmp/a"
  if command -v sha256sum >/dev/null 2>&1; then
    sum=$(sha256sum -b "$tmp/a" | awk '{printf $1}')
  else
    sum=$(shasum -a 256 -b "$tmp/a" | awk '{printf $1}')
  fi
  sh "$inst" verify_checksum "$tmp/a" sha256 "$sum" >/dev/null 2>&1 || fail "a matching checksum was refused"
  if sh "$inst" verify_checksum "$tmp/a" sha256 0000 >/dev/null 2>&1; then fail "a wrong checksum passed"; fi
  if sh "$inst" verify_checksum "$tmp/a" sha256 "" >/dev/null 2>&1; then fail "a missing checksum passed"; fi
  if sh "$inst" verify_checksum "$tmp/a" md5 "$sum" >/dev/null 2>&1; then fail "an unknown style passed"; fi
  if sh "$inst" fetch "$tmp/a" "" "" >/dev/null 2>&1; then fail "an archive without a checksum style passed"; fi

  # With no sha256sum and no shasum on PATH, the installer refuses.
  mkdir -p "$tmp/bin"
  for tool in awk sh; do ln -s "$(command -v "$tool")" "$tmp/bin/$tool"; done
  if PATH="$tmp/bin" sh "$inst" verify_checksum "$tmp/a" sha256 "$sum" >/dev/null 2>&1; then
    fail "verification without any SHA-256 tool passed"
  fi

  # A template drift (a rewrite target missing) fails closed and leaves the file.
  fixture | grep -v 'no checksums to verify' > "$tmp/drift.sh"
  cp "$tmp/drift.sh" "$tmp/drift.orig"
  if harden "$tmp/drift.sh" 2>/dev/null; then fail "a drifted template was accepted"; fi
  cmp -s "$tmp/drift.sh" "$tmp/drift.orig" || fail "a refused harden modified the file"

  # A second pass over a hardened file also fails closed (nothing to rewrite).
  if harden "$inst" 2>/dev/null; then fail "an already hardened installer was rewritten again"; fi

  echo "harden-installer self-test: ok"
}

case "${1:-}" in
  --self-test) self_test ;;
  ""|-*) echo "usage: harden-installer.sh <installer.sh> | --self-test" >&2; exit 2 ;;
  *) harden "$1" && echo "harden-installer: hardened $1" ;;
esac
