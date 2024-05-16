# Contributing

## Setup

- Set up the git pre-commit hook:

  ```bash
  pip install pre-commit
  pre-commit install
  ```

- [Install the nightly release of Rust](https://rustup.rs):

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain nightly
  ```

:::{note}
If you are new to Rust, see the [Reference section](https://ocp-software-handbook.readthedocs.io/en/latest/rust/#reference) of the Rust page of the OCP Software Development Handbook.
:::

## Benchmarks

```bash
cargo bench
```

:::{note}
We can consider the [criterion](https://crates.io/crates/criterion) crate for additional benchmarks.
:::

## Tasks

:::{toctree}
:maxdepth: 1

indicators/index
translation
:::
