# Setup the repository

## Develop the methodology

:::{admonition} To do
:class: caution
This section is a stub.
:::

All fences (thresholds) are inclusive (rather than exclusive) in Cardinal, to be consistent and to be easy to interpret. However, this means that, if the methodology identifies outliers using the interquartile range, and if the interquartile range is zero, then the indicator will always return a result. Therefore, the methodology must guard against this by returning nothing if the interquartile range is zero.

## Assign a code

When adding an indicator that is not assigned a code among the [resources](https://www.open-contracting.org/resources/) of the Open Contracting Partnership (or if you don't know):

- [ ] [Create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues) to be assigned a code.

(indicators-boilerplate)=
## Add boilerplate content

:::{admonition} One-time setup
To install the requirements for automation, create a Python virtual environment and run:

```bash
pip install click
```
:::

To perform these steps, run, replacing `r999`:

```bash
./manage.py add-indicator r999
```

The files created are explained in the next sections.

- [ ] Create the new module, `src/indicators/r999.rs`:

  :::{literalinclude} templates/rs
  :language: rust
  :::

- [ ] Create the test input, `tests/fixtures/indicators/R999.jsonl`:

  :::{literalinclude} templates/jsonl
  :language: json
  :::

- [ ] Create the test output, `tests/fixtures/indicators/R999.expected`:

  :::{literalinclude} templates/expected
  :language: json
  :::

- [ ] Create the documentation page, `docs/cli/indicators/R/999.md`:

  :::{literalinclude} templates/md
  :language: md
  :::

- [ ] Create the demonstration input, `docs/examples/R/999.jsonl`:

  :::{literalinclude} templates/demo
  :language: json
  :::

- In `src/indicators/mod.rs`:

  - [ ] Declare the new module in alphabetical order at the top of the file:

    ```rust
    pub mod r999;
    ```

  - [ ] Add a field to the `Settings` struct. This field is explained in the next section.

    ```rust
        pub R999: Option<Empty>,
    ```

  - [ ] Add a variant to the `Indicator` enum. The variants are keys in the JSON output of the {doc}`../../cli/indicators/index` command:

    ```rust
        R999,
    ```

- In `src/lib.rs`:

  - [ ] Import the new struct from the new module in alphabetical order at the top of the file:

    ```rust
    use crate::indicators::r999::R999;
    ```

  - [ ] Add the new struct in alphabetical order to the ``add_indicators!`` macro call:

    ```rust
                R999,
    ```

- [ ] In `docs/examples/settings.ini`, add a section for the new indicator in alphabetical order:

    ```ini
    [R999]
    ```

:::{admonition} Try it!
:class: tip

If you run:

```bash
cargo test
```

All tests should pass! (with warnings about unused variables and imports)
:::

:::{admonition} Next step
Now, you can {doc}`code the indicator<code>`.
:::
