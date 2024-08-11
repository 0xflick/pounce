#!/usr/bin/env bash

check_dependencies() {
  for cmd in git cargo fastchess; do
    if ! command -v $cmd &>/dev/null; then
      echo "Error: $cmd is not installed. Please install it before running this script."
      exit 1
    fi
  done
}

# Function to build a release binary from a specific commit
build_binary() {
  local commit=$1
  local output_binary=$2

  echo "Checking out commit $commit..."
  git checkout $commit || exit 1

  echo "Building release binary for commit $commit..."
  cargo build --release || exit 1

  # Move the binary to the output location
  mv target/release/flichess $output_binary || exit 1
}

cleanup() {
  echo "Returning to the original branch/commit..."
  git checkout "$initial_branch"
  echo "Cleaning up..."
  rm -f $binary1 $binary2 config.json
}

# Set trap to run cleanup on exit, error, or interrupt
trap cleanup EXIT

initial_branch=$(git rev-parse --abbrev-ref HEAD)
if [ "$initial_branch" == "HEAD" ]; then
  initial_branch=$(git rev-parse HEAD)
fi

if [ "$#" -ne 4 ]; then
  echo "Usage: $0 <commit1> <commit2> <elo0> <elo1>"
  exit 1
fi

# Store the commit hashes
commit1=$1
commit2=$2
elo0=$3
elo1=$4

# Check for required commands
check_dependencies

# Build binaries for each commit
binary1="bin_${commit1//\//_}"
binary2="bin_${commit2//\//_}"

build_binary $commit1 $binary1
build_binary $commit2 $binary2

echo "Running SPRT test between $binary1 and $binary2..."
fastchess \
  -engine cmd=$binary1 name=$commit1 \
  -engine cmd=$binary2 name=$commit2 \
  -sprt elo0=$elo0 elo1=$elo1 alpha=0.05 beta=0.05 \
  -each tc=10+0.1 -rounds 15000 -repeat -concurrency 12 -recover \
  -openings file=books/8moves_v3.pgn format=pgn

echo "SPRT test completed."
