# How to start rCore with aarch64

rust toolchain
```bash
curl https://sh.rustup.rs -sSf | sh
# proxy
export RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static
export RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup
rustup component add rust-src
cargo install cargo-xbuild bootimage
```

other toolchain
```bash
wget https://download.qemu.org/qemu-3.1.0.tar.xz && tar xvJf qemu-3.1.0.tar.xz
cd qemu-3.1.0 && ./configure --target-list=aarch64-softmmu && make && cd ..
# 配置环境变量，修改bashrc,添加一行
# export PATH=$地址/qemu-3.1.0/aarch64-softmmu:$地址/qemu-3.1.0:$PATH

export FILE="gcc-arm-8.2-2018.11-x86_64-aarch64-elf"; wget https://developer.arm.com/-/media/Files/downloads/gnu-a/8.2-2018.11/$FILE.tar.xz; tar -xvf $FILE.tar.xz; export PATH=$PATH:$PWD/$FILE/bin; wget https://musl.cc/aarch64-linux-musl-cross.tgz; tar -xvf aarch64-linux-musl-cross.tgz; export PATH=$PATH:$PWD/aarch64-linux-musl-cross/bin;
# 配置环境变量，把刚才两个bin加进去
```

clone project
```
git clone git@github.com:gaotianyu1350/rCore_audio.git --recursive
```

make user program
```
cd user
make sfsimg arch=aarch64
```

make kernel
```
cd ../kernel
make run arch=aarch64
```
