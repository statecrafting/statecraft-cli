#!/bin/sh
# Spec 057 R-2 and V-1: the release set verifies, or nothing is published.
#
# Given the directory the release workflow assembled, refuses unless, for each
# of the four targets of section 3.2, the archive and its .sha256 sidecar are
# present and agree, its CycloneDX SBOM has at least one component, and its
# build-provenance attestation verifies with `gh attestation verify` both
# against the bundle beside it and against the repository's attestation store;
# and unless the shell installer is hardened and carries every archive's
# SHA-256, and the Homebrew formula carries every archive's SHA-256.
#
# Usage: verify-set.sh <dir> <owner/repo>
#        verify-set.sh --self-test
set -eu

TARGETS="aarch64-apple-darwin x86_64-apple-darwin aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{ print $1 }'
  else
    shasum -a 256 "$1" | awk '{ print $1 }'
  fi
}

verify() {
  dir=$1 repo=$2 failures=0
  fail() { echo "verify-set: $1" >&2; failures=$((failures + 1)); }

  installer="$dir/statecraft-cli-installer.sh"
  formula="$dir/statecraft-cli.rb"
  for f in "$installer" "$installer.sha256" "$formula"; do
    [ -s "$f" ] || fail "missing or empty: $f"
  done

  for t in $TARGETS; do
    archive="$dir/statecraft-cli-$t.tar.xz"
    missing=0
    for f in "$archive" "$archive.sha256" "$archive.intoto.jsonl" "$dir/statecraft-cli-$t.cdx.json"; do
      [ -s "$f" ] || { fail "missing or empty: $f"; missing=1; }
    done
    [ "$missing" -eq 0 ] || continue

    want=$(awk 'NR == 1 { print tolower($1) }' "$archive.sha256")
    got=$(sha256_of "$archive")
    [ "$want" = "$got" ] || fail "$archive is $got, its sidecar says $want"

    n=$(jq '(.components // []) | length' "$dir/statecraft-cli-$t.cdx.json" 2>/dev/null || echo 0)
    [ "${n:-0}" -gt 0 ] || fail "SBOM for $t has zero components"

    gh attestation verify "$archive" --bundle "$archive.intoto.jsonl" --repo "$repo" >/dev/null \
      || fail "attestation does not verify against its bundle: $archive"
    gh attestation verify "$archive" --repo "$repo" >/dev/null \
      || fail "attestation does not verify against the attestation store: $archive"

    if [ -s "$installer" ] && ! grep -F -q "$got" "$installer"; then
      fail "the installer does not carry the SHA-256 of $t"
    fi
    if [ -s "$formula" ] && ! grep -F -q "$got" "$formula"; then
      fail "the formula does not carry the SHA-256 of $t"
    fi
  done

  if [ -s "$installer" ]; then
    grep -F -q 'ensure install -m 755 ' "$installer" || fail "the installer is not hardened (no install -m 755)"
    if grep -F -q 'skipping sha256 checksum verification' "$installer"; then
      fail "the installer can skip SHA-256 verification"
    fi
    if [ -s "$installer.sha256" ]; then
      want=$(awk 'NR == 1 { print tolower($1) }' "$installer.sha256")
      [ "$want" = "$(sha256_of "$installer")" ] || fail "the installer does not match its sidecar"
    fi
  fi

  if [ "$failures" -ne 0 ]; then
    echo "verify-set: $failures check(s) failed; nothing may be published" >&2
    return 1
  fi
  echo "verify-set: four archives, sidecars, SBOMs and attestations, the installer and the formula verify"
}

self_test() {
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  stfail() { echo "verify-set self-test: $1" >&2; exit 1; }
  command -v jq >/dev/null 2>&1 || stfail "jq is required"

  # A stub gh: an attestation verifies unless its archive is named in $tmp/bad.
  mkdir -p "$tmp/bin" "$tmp/set"
  cat > "$tmp/bin/gh" <<STUB
#!/bin/sh
[ "\$1 \$2" = "attestation verify" ] || exit 2
grep -F -x -q "\$3" "$tmp/bad" 2>/dev/null && exit 1
exit 0
STUB
  chmod 755 "$tmp/bin/gh"
  : > "$tmp/bad"

  set_dir="$tmp/set"
  { echo '#!/bin/sh'; echo 'ensure install -m 755 "$_src_dir/$_bin_name" "$_install_temp/$_bin_name"'; } > "$set_dir/statecraft-cli-installer.sh"
  echo 'class StatecraftCli < Formula' > "$set_dir/statecraft-cli.rb"
  for t in $TARGETS; do
    a="$set_dir/statecraft-cli-$t.tar.xz"
    printf 'archive %s' "$t" > "$a"
    s=$(sha256_of "$a")
    echo "$s  statecraft-cli-$t.tar.xz" > "$a.sha256"
    echo '{"bundle":1}' > "$a.intoto.jsonl"
    echo '{"components":[{"name":"serde"}]}' > "$set_dir/statecraft-cli-$t.cdx.json"
    echo "# $s" >> "$set_dir/statecraft-cli-installer.sh"
    echo "  sha256 \"$s\"" >> "$set_dir/statecraft-cli.rb"
  done
  echo "$(sha256_of "$set_dir/statecraft-cli-installer.sh")  statecraft-cli-installer.sh" > "$set_dir/statecraft-cli-installer.sh.sha256"

  run() { PATH="$tmp/bin:$PATH" verify "$1" example/repo >/dev/null 2>&1; }
  run "$set_dir" || stfail "a complete set was refused"

  case_dir() { rm -rf "$tmp/case"; cp -R "$set_dir" "$tmp/case"; : > "$tmp/bad"; }

  case_dir; echo '{"components":[]}' > "$tmp/case/statecraft-cli-x86_64-apple-darwin.cdx.json"
  if run "$tmp/case"; then stfail "an empty SBOM passed"; fi
  case_dir; rm "$tmp/case/statecraft-cli-aarch64-unknown-linux-gnu.tar.xz.intoto.jsonl"
  if run "$tmp/case"; then stfail "an archive without an attestation bundle passed"; fi
  case_dir; echo "$tmp/case/statecraft-cli-aarch64-apple-darwin.tar.xz" > "$tmp/bad"
  if run "$tmp/case"; then stfail "an attestation that does not verify passed"; fi
  case_dir; printf 'tampered' >> "$tmp/case/statecraft-cli-x86_64-unknown-linux-gnu.tar.xz"
  if run "$tmp/case"; then stfail "an archive that differs from its sidecar passed"; fi
  case_dir; rm "$tmp/case/statecraft-cli.rb"
  if run "$tmp/case"; then stfail "a set without the formula passed"; fi
  case_dir; grep -v 'install -m 755' "$set_dir/statecraft-cli-installer.sh" > "$tmp/case/statecraft-cli-installer.sh"
  if run "$tmp/case"; then stfail "an unhardened installer passed"; fi

  echo "verify-set self-test: ok"
}

case "${1:-}" in
  --self-test) self_test ;;
  ""|-*) echo "usage: verify-set.sh <dir> <owner/repo> | --self-test" >&2; exit 2 ;;
  *) verify "$1" "${2:?usage: verify-set.sh <dir> <owner/repo>}" ;;
esac
