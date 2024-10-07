# Compiling it yourself (advanced)

Compiling the application should only be done when you are comfortable of doing it yourself.

## Steps

Set up a [Rust environment](https://www.rust-lang.org/tools/install). The latest version of Rust should work. (Rust versions ≥ 1.80, < 2.0 are supported.)

Once you have Rust installed, use the cargo build tool:

```sh
cargo build --release
```

The program will be placed in the `target` directory. You can run it as is, or install it by adding a "-installer" suffix to the filename before running it.