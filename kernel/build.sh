#!/bin/bash
export RUSTFLAGS="$RUSTFLAGS -A warnings "
export SDL_AUDIODRIVER=none
make run arch=x86_64 graphic=on
