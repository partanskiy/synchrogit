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
  echo "Homebrew publishing skips semver prerelease/build metadata: $version" >&2
  exit 1
fi

download_and_sha256() {
  local url=$1
  local output=$2

  curl -LfsS "$url" -o "$tmpdir/$output"
  sha256sum "$tmpdir/$output" | awk '{ print $1 }'
}

tag="v${version}"
mac_arm64_url="${repo_url}/releases/download/${tag}/synchrogit-${tag}-aarch64-apple-darwin.tar.gz"
mac_x86_64_url="${repo_url}/releases/download/${tag}/synchrogit-${tag}-x86_64-apple-darwin.tar.gz"

mac_arm64_sha=${MAC_ARM64_SHA256:-$(download_and_sha256 "$mac_arm64_url" "synchrogit-arm64-darwin.tar.gz")}
mac_x86_64_sha=${MAC_X86_64_SHA256:-$(download_and_sha256 "$mac_x86_64_url" "synchrogit-x86_64-darwin.tar.gz")}

mkdir -p "$output_root"

sed \
  -e "s|@PKGVER@|$version|g" \
  -e "s|@REPO_URL@|$repo_url|g" \
  -e "s|@MAC_ARM64_URL@|$mac_arm64_url|g" \
  -e "s|@MAC_ARM64_SHA256@|$mac_arm64_sha|g" \
  -e "s|@MAC_X86_64_URL@|$mac_x86_64_url|g" \
  -e "s|@MAC_X86_64_SHA256@|$mac_x86_64_sha|g" \
  "$repo_root/packaging/brew/synchrogit.rb.in" > "$output_root/synchrogit.rb"
