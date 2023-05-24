# Add an indicator

You can use the check marks to track your progress (do not reload the page).

## Assign a code

Indicators are assigned codes for easy reference: for example, `R001`.

The first letter indicates the category: **R**ed flag or **U**se case.

When adding an indicator that is not assigned a code among the [resources](https://www.open-contracting.org/resources/) of the Open Contracting Partnership (or if you don't know), [create an issue on GitHub](http://github.com/open-contracting/cardinal-rs/issues) to be assigned a code.

In this tutorial, the example indicator is given the code `R999`.

## Add boilerplate content

````{admonition} One-time setup
To install the requirements for automation, create a Python virtual environment and run:

```bash
pip install click
```
````

To perform these steps, run, for example:

```bash
./manage.py add-indicator r999
```

- [ ] Create the new module, `src/indicators/r999.rs`:

  ```{literalinclude} templates/rs
  :language: rust
  ```

- [ ] Create the test input, `tests/fixtures/indicators/R999.jsonl`:

  ```{literalinclude} templates/jsonl
  :language: json
  ```

- [ ] Create the test output, `tests/fixtures/indicators/R999.expected`:

  ```{literalinclude} templates/expected
  :language: json
  ```

- In `src/indicators/mod.rs`:

  - [ ] Declare the new module in alphabetical order at the top of the file:

    ```rust
    pub mod r999;
    ```

  - [ ] Add a field to the `Settings` struct. Re-use existing structs to model the indicator's settings, if possible. If the indicator accepts no options, or you don't know, use a `HashMap`.

    ```rust
        pub R999: Option<HashMap<String, String>>,
    ```

  - [ ] Add a variant to the `Indicator` enum. The variants are keys in the JSON output of the {doc}`../cli/indicators/index` command:

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

````{note}

If you run:

```bash
cargo test
```

All tests should pass! (with warnings about unused variables and imports)
````

## Add documentation

- [ ] Create the new page, `docs/cli/indicators/R/999.md`:

  ```{literalinclude} templates/md
  :language: md
  ```

- [ ] In `docs/cli/indicators/index.md`, add the new indicator in alphabetical order to the relevant table:

  ```md
  * - [R999](R/999)
    - [The title of the indicator](R/999)
  ```


- [ ] In `docs/changelog.md`, add a changelog entry for the new indicator.

  ```md
  ### Added

  - {doc}`cli/indicators/index` command:

    - R999 (*The title of the indicator*).
