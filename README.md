# rCore

[![Build Status](https://travis-ci.org/rcore-os/rCore.svg?branch=master)](https://travis-ci.org/rcore-os/rCore)

Rust version of THU [uCore OS Plus](https://github.com/chyyuu/ucore_os_plus).

Going to be the next generation teaching operating system.

Supported architectures: x86_64, RISCV32/64, AArch64, MIPS32

Tested boards: QEMU, HiFive Unleashed, x86_64 PC (i5/i7), Raspberry Pi 3B+, Kendryte K210 and FPGA running Rocket Chip / [TrivialMIPS](https://github.com/Harry-Chen/TrivialMIPS)

![demo](./docs/2_OSLab/os2atc/demo.png)

## Building

### Environment

* [Rust](https://www.rust-lang.org) toolchain at nightly-2019-03-05
* Cargo tools: [cargo-xbuild](https://github.com/rust-osdev/cargo-xbuild)
* [QEMU](https://www.qemu.org) >= 3.1.0
* [bootimage](https://github.com/rust-osdev/bootimage) (for x86_64)
* [RISCV64 GNU toolchain](https://www.sifive.com/boards) (for riscv32/64)
* [AArch64 GNU toolchain](https://cs140e.sergio.bz/assignments/0-blinky/) (for aarch64)
* [musl-cross-make](https://github.com/richfelker/musl-cross-make) (for userland musl, or download prebuilt toolchain from [musl.cc](https://musl.cc/))
* [libfuse-dev](https://github.com/libfuse/libfuse) (for userland image generation)

See [Travis script](./.travis.yml) for details.

### How to run

```bash
$ rustup component add rust-src llvm-tools-preview
$ cargo install cargo-binutils
$ cargo install cargo-xbuild --force
$ cargo install bootimage --version 0.5.7 --force
```

```bash
$ git clone https://github.com/rcore-os/rCore.git --recursive
$ cd rCore/user
$ make sfsimg arch={riscv32,riscv64,x86_64,aarch64,mipsel} # requires $(arch)-linux-musl-gcc
$ cd ../kernel
$ make run arch={riscv32,riscv64,x86_64,aarch64,mipsel} mode=release
$ make run arch=x86_64 mode=release pci_passthru=0000:00:00.1 # for ixgbe real nic, find its pci (bus, dev, func) first
```

## Maintainers

| Module | Maintainer            |
|--------|-----------------------|
| x86_64 | @wangrunji0408        |
| RISC-V  | @jiegec               |
| ARM (Raspi3) | @equation314    |
| MIPS   | @Harry_Chen @miskcoo   |
| Memory, Process, File System | @wangrunji0408          |
| Network with drivers | @jiegec |
| GUI    | @equation314          |

## History

This is a project of THU courses:

* [Operating System (2018 Spring) ](http://os.cs.tsinghua.edu.cn/oscourse/OS2018spring/projects/g11)
* [Comprehensive Experiment of Computer System (2018 Summer)](http://os.cs.tsinghua.edu.cn/oscourse/csproject2018/group05)
* [Operating System Train (2018 Autumn)](http://os.cs.tsinghua.edu.cn/oscourse/OsTrain2018)
* [Operating System (2019 Spring)](http://os.cs.tsinghua.edu.cn/oscourse/OS2019spring/projects)

[Reports](./docs) and [Dev docs](https://rucore.gitbook.io/rust-os-docs/) (in Chinese)

It's based on [BlogOS](https://github.com/phil-opp/blog_os) , a demo project in the excellent tutorial [Writing an OS in Rust (First Edition)](https://os.phil-opp.com/first-edition/).

## License

The source code is dual-licensed under MIT or the Apache License (Version 2.0).
