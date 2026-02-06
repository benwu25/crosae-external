# DATIR: Dynamic Abstract Type Inference in Rust
This repository contains all the code necessary to automatically insert instrumentation into arbitrary Rust source code, to perform abstract type inference (ATI). This project is loosely based on [dynamic inference of abstract types](https://dl.acm.org/doi/10.1145/1146238.1146268).

## Using This Repository
This instrumentation relies on `rustc`'s query system to execute callbacks. This requires linking against `rustc`'s nightly build. To do so, run the following bash [commands](https://rust-lang.github.io/rustup/installation/index.html):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none -y;

echo ". \"$HOME/.cargo/env\"" >> ~/.bashrc;

rustup toolchain install nightly --allow-downgrade --profile minimal --component clippy;

rustup component add rust-src rustc-dev llvm-tools-preview;
```

At this point, you should be able to compile and run this project with:
```sh
cargo run [OPTIONAL] -- INPUT [OPTIONAL]
```

Optional arguments passed before the `--` are passed to the `rustc` invocation responsible for building the instrumentation compiler. Optional arguments passed after are forwarded to the compiler invocation when instrumenting `INPUT`.

Note that if this project is built into a binary, it requires extra linking with `rustc`'s private library to execute, by setting the `LD_LIBRARY_PATH` environment variable to point to the nightly compiler build (e.g. `$REPO_HOME/target/debug/deps:$HOME/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib`). Until this is resolved, it's easiest to stick with the `cargo run` option mentioned above.

## File Description
The following files make up the majority of the implementation:

1. `src/ati/*`: Contains the ATI library that is used at runtime to dynamically keep track of value interactions.
2. `src/callbacks/*`: Defines the callbacks used by various compiler invocations. DATIR currently relies on being able to perform two compilations, one to generally gather some information, another to perform the actual instrumentation.
3. `src/file_loaders/*`: Defines a custom FileLoader which is capable of performing AST-level mutations before the file contents even make it to the compiler parser.
4. `src/types/*`: Defines helpful collections of data used throughout the project.
4. `src/visitors/*`: Defines the visitors which mutate or discover information from various IRs. These visitors are ultimately orchestrated by the callbacks or FileLoaders.
6. `src/common/*`: Miscellaneous helper functions, used throughout.
7. `tests/*`: Unit tests, which invoke the compiler on input files and checks the ATI output against an expected partition.

## Output
The exact form of output is governed by `src/ati/ati.rs::ATI::report()`. This function is invoked right before `main` exits. Currently, the output is just written to stdout, starting with `===ATI-ANALYSIS-START===`. For example, the following output is produced by instrumenting and executing a simple program which has `main` invoke another function `foo`, which accepts three parameters `x`, `y`, and `z` (`tests/simple/input.rs`). The values of `x` and `y` are used to compute the return value, `RET`.

```
===ATI-ANALYSIS-START===
foo::ENTER
x:0
y:1
z:2
---
foo::EXIT
RET:0
x:0
y:0
z:2
---
main::ENTER
---
main::EXIT
---
```

This instrumentation only reports the abstract types of formals and return values, ultimately to construct a program specification.

## Features yet to be implemented:
The following is a list of features still in progress:

1. It is left unspecified how to handle complex return types from untracked functions.
2. Methods implemented on structs or enums
3. Enums?
4. Closures?
