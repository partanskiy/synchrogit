#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK directory}"
ndk="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}"
case "$(uname -s)" in
  Linux) host=linux-x86_64 ;;
  Darwin) host=darwin-x86_64 ;;
  *) echo 'Build Rust Android libraries on Linux or macOS (or WSL).' >&2; exit 1 ;;
esac
bin="$ndk/toolchains/llvm/prebuilt/$host/bin"
export PATH="$bin:$PATH"
export ANDROID_NDK_HOME="$ndk"
mkdir -p android/app/src/main/assets
cp THIRD_PARTY_LICENSES.html android/app/src/main/assets/
for abi in ${ANDROID_ABIS:-arm64-v8a x86_64}; do
  case "$abi" in
    arm64-v8a) target=aarch64-linux-android ;;
    x86_64) target=x86_64-linux-android ;;
    *) echo "Unsupported Android ABI: $abi" >&2; exit 1 ;;
  esac
  key="${target//-/_}"
  export "CC_${key}=$bin/${target}26-clang"
  export "AR_${key}=$bin/llvm-ar"
  export "CARGO_TARGET_${key^^}_LINKER=$bin/${target}26-clang"
  # r28 supports 16 KiB pages; make the Rust shared-library alignment explicit.
  export "CARGO_TARGET_${key^^}_RUSTFLAGS=-C link-arg=-Wl,-z,max-page-size=16384"
  cargo build --locked --release --lib --target "$target"
  mkdir -p "android/app/src/main/jniLibs/$abi"
  cp "target/$target/release/libsynchrogit.so" "android/app/src/main/jniLibs/$abi/"
done
