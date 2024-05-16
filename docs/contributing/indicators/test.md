# Write the tests

The test framework reads `*.jsonl` files in the `tests/fixtures/indicators/` directory. Each `*.jsonl` file is prefixed with an indicator code: for example, `R038.jsonl` and `R038_tenderer.jsonl`. The test framework runs the `indicators` command with that indicator {ref}`enabled<enable-an-indicator>`, and compares the output to the corresponding `*.expected` file: for example, `R038.expected` and `R038_tenderer.expected`.

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

:::{admonition} Next step
Finally, you can {doc}`add documentation<docs>`.
:::
