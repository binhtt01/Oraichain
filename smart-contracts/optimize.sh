#!/bin/bash
set -o errexit -o nounset -o pipefail
command -v shellcheck > /dev/null && shellcheck "$0"

export PATH=$PATH:/root/.cargo/bin

echo "Info: RUSTC_WRAPPER=$RUSTC_WRAPPER"

echo "Info: sccache stats before build"
sccache -s

contractdir=${1:-nl002}
parentdir=${2:-nlp}

mkdir -p $PWD/package/$parentdir/$contractdir/artifacts

ARTIFACTS=$PWD/package/$parentdir/$contractdir/artifacts

WASM_PATH=$PWD/target/wasm32-unknown-unknown/release/

# There are two cases here
# 1. All contracts (or one) are included in the root workspace  (eg. `cosmwasm-template`, `cosmwasm-examples`, `cosmwasm-plus`)
#    In this case, we pass no argument, just mount the proper directory.
# 2. Contracts are excluded from the root workspace, but import relative paths from other packages (only `cosmwasm`).
#    In this case, we mount root workspace and pass in a path `docker run <repo> ./contracts/hackatom`

# This parameter allows us to mount a folder into docker container's "/code"
# and build "/code/contracts/mycontract".
# Note: if contractdir is "." (default in Docker), this ends up as a noop
echo "Building contract in $(realpath -m "$contractdir")"
(

  # Linker flag "-s" for stripping (https://github.com/rust-lang/cargo/issues/3483#issuecomment-431209957)
  # Note that shortcuts from .cargo/config are not available in source code packages from crates.io
  RUSTFLAGS='-C link-arg=-s' cargo build --release --target wasm32-unknown-unknown
)

# wasm-optimize on all results
for wasm in $WASM_PATH/$contractdir.wasm; do
  name=$(basename "$wasm")
  echo "Optimizing $name"
  wasm-opt -Os "$wasm" -o $ARTIFACTS/$name
done

# create hash
(
  cd $ARTIFACTS/
  sha256sum -- *.wasm > checksums.txt
)

echo "Info: sccache stats after build"
sccache -s

echo "done"