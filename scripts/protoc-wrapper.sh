#!/bin/sh
set -eu

cargo_home="${CARGO_HOME:-$HOME/.cargo}"
platform_pattern='protoc-bin-vendored-*'

case "$(uname -s):$(uname -m)" in
  Darwin:arm64|Darwin:aarch64)
    platform_pattern='protoc-bin-vendored-macos-aarch_64-*'
    ;;
  Darwin:x86_64)
    platform_pattern='protoc-bin-vendored-macos-x86_64-*'
    ;;
  Linux:x86_64)
    platform_pattern='protoc-bin-vendored-linux-x86_64-*'
    ;;
  Linux:aarch64|Linux:arm64)
    platform_pattern='protoc-bin-vendored-linux-aarch_64-*'
    ;;
esac

protoc_path=$(find "$cargo_home/registry/src" -path "*/$platform_pattern/bin/protoc" | head -n 1)

if [ -z "$protoc_path" ]; then
  echo "Unable to locate vendored protoc in $cargo_home/registry/src" >&2
  exit 1
fi

exec "$protoc_path" "$@"
