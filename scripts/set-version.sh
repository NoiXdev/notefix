#!/usr/bin/env bash
set -euo pipefail

# Set the project version in every place that carries it, so a release is fully
# driven by the git tag (no hand-editing of multiple files). Used by the release
# workflow; safe to run locally too.
#
#   scripts/set-version.sh 0.2.0
#
# Touches: package.json (+ package-lock.json), src-tauri/tauri.conf.json,
# src-tauri/Cargo.toml ([package] version) and src-tauri/Cargo.lock (the notefix
# entry). Uses only node/npm/perl so it runs on any runner without a Rust
# toolchain.

V="${1:?usage: set-version.sh <version, e.g. 0.2.0>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# package.json + package-lock.json
npm version "$V" --no-git-tag-version --allow-same-version >/dev/null

# tauri.conf.json (drives the app + Windows/Linux artifact version)
node -e "const fs=require('fs');const f='src-tauri/tauri.conf.json';const c=JSON.parse(fs.readFileSync(f,'utf8'));c.version=process.argv[1];fs.writeFileSync(f,JSON.stringify(c,null,2)+'\n')" "$V"

# src-tauri/Cargo.toml — only the [package] version, not dependency versions.
V="$V" perl -0pi -e 's/(\[package\][^\[]*?\nversion = ")[^"]*(")/$1$ENV{V}$2/s' src-tauri/Cargo.toml

# src-tauri/Cargo.lock — the notefix entry (version string only; deps unchanged).
V="$V" perl -0pi -e 's/(name = "notefix"\nversion = ")[^"]*(")/$1$ENV{V}$2/' src-tauri/Cargo.lock

echo "version set to $V"
