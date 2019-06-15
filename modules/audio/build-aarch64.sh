#!/bin/bash
ARCH=aarch64
TEXT_TYPE=elf64-littleaarch64
BIN_ARCH=aarch64
PREFIX=aarch64-elf-
echo "Step 1. Fetching dependencies according to cargo."
echo "// Dummy file" > src/lib.rs
echo '#![no_std]' >> src/lib.rs
echo "extern crate rcore;" >> src/lib.rs
cargo xbuild --target=../../kernel/targets/$ARCH.json -vv --release
echo "Step 2. Compile the library"
echo '#![no_std]' > src/lib.rs
echo '#![feature(alloc)]' >> src/lib.rs
echo "extern crate rcore;" >> src/lib.rs
echo "mod main;" >> src/lib.rs
rustc --edition=2018 --crate-name audio src/lib.rs \
--color always --crate-type dylib  -C debuginfo=2 \
--out-dir ./target/$ARCH/release/objs \
--target ../../kernel/targets/$ARCH.json \
-L dependency=target/$ARCH/release/deps \
-L dependency=target/release/deps \
--emit=obj --sysroot target/sysroot \
-L all=../../kernel/target/$ARCH/release/deps \
-C relocation-model=pic
echo "Step 3. Packing the library into kernel module."
"$PREFIX"objcopy --input binary --output $TEXT_TYPE \
    --binary-architecture $BIN_ARCH\
    --rename-section .data=.rcore-lkm,CONTENTS,READONLY\
    lkm_info.txt target/$ARCH/release/objs/lkm_info.o
"$PREFIX"strip target/$ARCH/release/objs/lkm_info.o
"$PREFIX"gcc -shared -o target/$ARCH/release/audio.ko -nostdlib target/$ARCH/release/objs/*.o
#cargo xbuild --target=../../kernel/targets/x86_64.json -vv
