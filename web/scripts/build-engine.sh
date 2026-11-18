#!/bin/sh
# Builds the Design engine (bindings/wasm) to WebAssembly into public/engine (or $DESIGN_ENGINE_OUT),
# where the editor loads it from its own package. Needs wasm-pack, and a C compiler for wasm32
# (Homebrew's llvm clang on macOS) for the engine's C dependencies.
set -eu
web="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$web/.." && pwd)"
out="${DESIGN_ENGINE_OUT:-$web/public/engine}"
if toolchain="$(rustc --print sysroot 2>/dev/null)" && [ -d "$toolchain/lib" ]; then
  export DYLD_FALLBACK_LIBRARY_PATH="$toolchain/lib${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}"
fi
if [ -z "${CC_wasm32_unknown_unknown:-}" ] && command -v brew >/dev/null 2>&1 && [ -x "$(brew --prefix llvm 2>/dev/null)/bin/clang" ]; then
  export CC_wasm32_unknown_unknown="$(brew --prefix llvm)/bin/clang"
fi
# wasm-pack runs cargo inside the crate's folder: keep the target directory absolute.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target}"
cd "$repo"
wasm-pack build bindings/wasm --target web --release --no-typescript --out-dir "$out"
rm -f "$out/.gitignore" "$out/package.json"
# The faces the editor needs that the web build does not embed, from the kition-fonts checkout next
# to the repository (or KITION_FONTS_DIR): Droid Sans Fallback, which the engine gets through
# `addFont` for Chinese and Japanese text (never Noto CJK or Source Han), and Inter, the upstream
# editor's UI face. Everything still runs without them (system faces instead).
fonts="${KITION_FONTS_DIR:-$repo/../kition-fonts}/fonts"
fonts_out="$web/public/fonts"
rm -rf "$fonts_out"
mkdir -p "$fonts_out"
copy() { # <dir> <font file> <licence file> <licence name in the package>
  if [ -f "$fonts/$1/$2" ]; then
    cp "$fonts/$1/$2" "$fonts_out/"
    cp "$fonts/$1/$3" "$fonts_out/$4"
  else
    echo "warning: no $1/$2 in the kition-fonts checkout at $fonts" >&2
  fi
}
copy droid-sans-fallback DroidSansFallbackFull.ttf NOTICE DroidSansFallback-NOTICE.txt
copy inter Inter-Variable.ttf OFL.txt Inter-OFL.txt
