#!/bin/bash
echo "Step 1. Fetching dependencies according to cargo."
echo "// Dummy file" > src/lib.rs
echo '#![no_std]' >> src/lib.rs
echo "extern crate rcore;" >> src/lib.rs
cargo xbuild --target=../../kernel/targets/aarch64.json -vv
echo "Step 2. Compile the library"
echo '#![no_std]' > src/lib.rs
echo '#![feature(alloc)]' >> src/lib.rs
echo "extern crate rcore;" >> src/lib.rs
echo "mod main;" >> src/lib.rs
rustc --edition=2018 --crate-name hello_rust src/lib.rs \
--color always --crate-type cdylib  -C debuginfo=2 -C metadata=bf2857974bf47761 --out-dir ./objs \
--target /home/gjz010/rcore_plus/kernel/targets/aarch64.json  \
-L dependency=target/aarch64/debug/deps \
-L dependency=target/debug/deps \
--emit=obj --sysroot /home/gjz010/rcore_plus/modules/hello_rust/target/sysroot \
-L all=../../kernel/target/aarch64/release/deps \
-C relocation-model=pic
echo "Step 3. Packing the library into kernel module."
aarch64-elf-objcopy --input binary --output elf64-littleaarch64 --binary-architecture aarch64\
    --rename-section .data=.rcore-lkm,CONTENTS,READONLY\
    lkm_info.txt objs/lkm_info.o
aarch64-elf-strip objs/lkm_info.o
aarch64-elf-gcc -shared -o hello_rust.ko -nostdlib objs/*.o
cargo xbuild --target=../../kernel/targets/aarch64.json -vv
