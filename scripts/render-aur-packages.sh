#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 <version> <repo-url> <output-dir>" >&2
  exit 1
fi

version=$1
repo_url=${2%/}
output_root=$3
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tmpdir=$(mktemp -d)

cleanup() {
  rm -rf "$tmpdir"
}

trap cleanup EXIT

if [[ "$version" == v* ]]; then
  echo "version must not include the leading v: $version" >&2
  exit 1
fi

if [[ "$version" == *-* || "$version" == *+* ]]; then
  echo "AUR pkgver does not support semver prerelease/build metadata: $version" >&2
  exit 1
fi

download_and_sha256() {
  local url=$1
  local output=$2

  curl -LfsS "$url" -o "$tmpdir/$output"
  sha256sum "$tmpdir/$output" | awk '{ print $1 }'
}

generate_srcinfo() {
  local pkgdir=$1

  if [[ $EUID -eq 0 ]]; then
    chown -R nobody:nobody "$pkgdir"
    runuser -u nobody -- sh -lc "cd '$pkgdir' && makepkg --printsrcinfo > .SRCINFO"
  else
    (
      cd "$pkgdir"
      makepkg --printsrcinfo > .SRCINFO
    )
  fi
}

render_template() {
  local template=$1
  local output=$2

  sed \
    -e "s|@PKGVER@|$version|g" \
    -e "s|@REPO_URL@|$repo_url|g" \
    -e "s|@SOURCE_URL@|$source_url|g" \
    -e "s|@SOURCE_SHA256@|$source_sha|g" \
    -e "s|@BIN_X86_64_URL@|$bin_x86_64_url|g" \
    -e "s|@BIN_AARCH64_URL@|$bin_aarch64_url|g" \
    -e "s|@BIN_X86_64_SHA256@|$bin_x86_64_sha|g" \
    -e "s|@BIN_AARCH64_SHA256@|$bin_aarch64_sha|g" \
    "$template" > "$output"
}

tag="v${version}"
source_url="${repo_url}/archive/refs/tags/${tag}.tar.gz"
bin_x86_64_url="${repo_url}/releases/download/${tag}/synchrogit-${tag}-x86_64-unknown-linux-gnu.tar.gz"
bin_aarch64_url="${repo_url}/releases/download/${tag}/synchrogit-${tag}-aarch64-unknown-linux-gnu.tar.gz"

source_sha=${SOURCE_SHA256:-$(download_and_sha256 "$source_url" "synchrogit-source.tar.gz")}
bin_x86_64_sha=${BIN_X86_64_SHA256:-$(download_and_sha256 "$bin_x86_64_url" "synchrogit-x86_64-linux.tar.gz")}
bin_aarch64_sha=${BIN_AARCH64_SHA256:-$(download_and_sha256 "$bin_aarch64_url" "synchrogit-aarch64-linux.tar.gz")}

mkdir -p "$output_root/synchrogit" "$output_root/synchrogit-bin"

render_template \
  "$repo_root/aur/synchrogit/PKGBUILD.in" \
  "$output_root/synchrogit/PKGBUILD"

render_template \
  "$repo_root/aur/synchrogit-bin/PKGBUILD.in" \
  "$output_root/synchrogit-bin/PKGBUILD"

generate_srcinfo "$output_root/synchrogit"
generate_srcinfo "$output_root/synchrogit-bin"
