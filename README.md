# Rust VB Platform

A set of Rust packages used to build Virtual Boy games. Heavily WIP.

This project depends on the [v810-rust toolchain](https://github.com/SupernaviX/v810-rust). You must install that to use it.

For simple usage examples, see the [examples](./examples/) directory. For convenience, this project has a `justfile` (think of `just` as a hipster `make` that works on Windows). You can compile an example by running `just build hello-world`, or see the emitted code by running `just assembly hello-world`.

For a more complex project, see the source code for [Virtual Picross](https://github.com/SupernaviX/virtual-picross).

# Packages

`vb-rt`: The core runtime. Handles all initialization. Exposes useful hardware addresses through `vb_rt::sys`.
`vb-rt-build`: A build dependency for use with `vb-rt`, responsible for configuring the linker. Use it in your `build.rs` file.

`vb-graphics`: A simple graphical library. Display images as backgrounds or objects, render text, handle frame timings, all that good stuff.
`vb-graphics-build`: A build dependency for use with `vb-graphics`, which compiles PNGs and TTFs into formats that the graphics library can use. Configured by a file named `assets.toml` in your project's root. Use it in your `build.rs` file.