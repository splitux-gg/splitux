#!/bin/bash

cargo build --release && \
rm -rf build/splitux
mkdir -p build/ build/res build/bin && \
cp target/release/splitux build/ && \
cp LICENSE build/ && cp COPYING.md build/thirdparty.txt && \
cp res/splitscreen_kwin.js res/splitscreen_kwin_vertical.js build/res && \
cp deps/gamescope/build-gcc/src/gamescope build/bin/gamescope-kbm
