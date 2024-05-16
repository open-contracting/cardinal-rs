# Write the tests

The test framework (in `build.rs`) reads `*.jsonl` files in the `tests/fixtures/indicators/` directory. Each `*.jsonl` file is prefixed with an indicator code: for example, `R038.jsonl` and `R038-tenderer.jsonl`. The test framework runs the `indicators` command with that indicator {ref}`enabled<enable-an-indicator>`, and compares the output to the corresponding `*.expected` file: for example, `R038.expected` and `R038-tenderer.expected`.

The test file(s) for an indicator should:

- Cause the indicator to return a result (to test for true positives)
- Include data that covers any {ref}`exclusions<indicators-exclusions>` or edge cases (to test against false positives)
- Not include data that is irrelevant to the indicator (to make the test readable)

You can use the `ocid` field to describe each compiled release.

You can use single letters for identifiers, for example:

:::{hlist}
:columns: 2

- Identifier to be **F**lagged
- **B**uyer
- **P**rocuring entity
- **S**upplier of an active award
- Supplier of a **C**ancelled award
- Supplier of an **U**nsuccessful award
- Tenderer of a **I**nvited bid
- Tenderer of a **P**ending bid
- Tenderer of a **V**alid bid
- Tenderer of a **D**isqualified bid
- **W**inning tenderer
- **L**osing tenderer
:::

:::{admonition} Example
:class: seealso

Edit the `R999.jsonl` and `R999.expected` files in the `tests/fixtures/indicators/` directory from {ref}`indicators-boilerplate`. To only test whether the indicator returns a result, replace the contents of the files with:

```json
{"ocid":"F","bids":{"details":[{"status":"valid"}]},"awards":[{"status":"active"}]}
```

And:

```json
{"OCID":{"F":{"R999":1.0}}}
```
:::

:::{admonition} Try it!
:class: tip

If you run:

```bash
cargo test
```

All tests should pass, including:

```none
test tests::r999 ... ok
```
:::

## Advanced tests

If the indicator has many {ref}`configurations<indicators-boilerplate>`, add test files for each configuration.

When the test framework reads `*.jsonl` files in the `tests/fixtures/indicators/` directory, it splits the basename on hyphens (`-`) into parts. The first part is the indicator code.

If one part remains, this last part is ignored. It serves only to describe the test case. Example:

```none
R038-tenderer.jsonl
```

If two parts remain, the second part is the configuration field, and the third part is the configuration value. If the third part is only digits, it is used as an integer. Otherwise, it is used as a string.

```none
R048-minimum_contracting_processes-1.jsonl
```

If more than two parts remain, the second part is the configuration field. The remaining parts are key-value pairs for a HashMap, in which the keys are strings and the values are integers.

```none
R003-procurement_method_details-x-5-y-10.jsonl
```

:::{note}
To use a pipe-separated list, replace the pipe with a plus sign (`+`).

```none
R003-procurement_method-open+selective.jsonl
```
:::

:::{tip}
If you need more flexibility (for example, `f64`), [create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues).
:::

:::{admonition} Next step
Finally, you can {doc}`add documentation<docs>`.
:::
