# Code an indicator

(indicators-settings)=
## Edit the settings in `src/indicators/mod.rs`

The {ref}`configurations<indicators-config>` for an indicator are represented as a field named after the indicator (`R999`) on the `Settings` struct, defined in `src/indicators/mod.rs`.

:::{literalinclude} ../../../src/indicators/mod.rs
:language: rust
:start-at: struct Settings
:end-at: "}"
:::

In Cardinal, all configurations are optional. So, the field must be an [`Option<T>`](https://doc.rust-lang.org/std/option/index.html), and the fields on the struct that the `Option` contains (`T`) must also be optional.

If the indicator's only configuration is a threshold (integer or decimal), then the `IntegerThreshold` or `FloatThreshold` struct can be used, shown below for easy reference.

:::{literalinclude} ../../../src/indicators/mod.rs
:language: rust
:start-at: struct IntegerThreshold
:end-at: "}"
:::

:::{literalinclude} ../../../src/indicators/mod.rs
:language: rust
:start-at: struct FloatThreshold
:end-at: "}"
:::

If the indicator has no configuration, the `Empty` struct can be used, which has no fields.

:::{literalinclude} ../../../src/indicators/mod.rs
:language: rust
:start-at: struct Empty
:end-at: "}"
:::

Otherwise, create a new struct named after the indicator. For example:

```rust
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct R999 {
    my_integer: Option<usize>,
    my_decimal: Option<f64>,
    my_text: Option<String>,
}
```

The [`#[serde(deny_unknown_fields)]`](https://serde.rs/container-attrs.html#deny_unknown_fields) attribute causes an error if the user sets an unknown property.

:::{admonition} Example
:class: seealso

R999's methodology is "A competition completed with few submitted bids." You will edit the settings to allow users to configure the number of submitted bids (the "threshold") that raises the red flag.

In `src/indicators/mod.rs`, the `Settings` struct already has a field for the indicator from {ref}`indicators-boilerplate`:

```rust
    pub R999: Option<Empty>,
```

As is, no configuration is allowed. Cardinal attempts to parse any properties in the `[R999]` section of the INI file into the `Empty` struct. Because the struct has no fields, no properties are parsed, and the user sees an error as feedback.

The number of submitted bids can be represented as an integer. To parse a property with the name `threshold` and an integer value, you can reuse the `IntegerThreshold` struct:

```rust
    pub R999: Option<IntegerThreshold>,
```

Users can now configure R999's threshold, using the {doc}`../../topics/settings`. For example:

```ini
[R999]
threshold = 2
```
:::

:::{admonition} Try it!
:class: tip

Follow the example, create a `settings.ini` file with the content above, and run:

```bash
echo '{}' | cargo run -- indicators --settings settings.ini -
```

The output should be `{}`, with no errors about unknown fields!
:::

## Write the module

Open the new module (`src/indicators/r999.rs`, in this example) in a text editor.

An indicator is an `impl`ementation of the `Calculate` trait on a struct (`R999`, in this example).

:::{literalinclude} templates/rs
:language: rust
:lines: 5-9
:emphasize-lines: 2,5
:::

Note that items (like structs) are scoped by their module. In other words, an `R999` struct in `mod.rs` for the indicator's configuration has no relation with the `R999` struct in `r999.rs` for its internal state.

:::{hint}
Comparing Rust to other languages, [structs](https://doc.rust-lang.org/book/ch05-00-structs.html) are like objects, and [traits](https://doc.rust-lang.org/book/ch10-02-traits.html) are like interfaces. Structs have data ("fields"), and `impl` blocks provide a struct's methods. Like Python, items are scoped by module and are imported (`use`).
:::

The `Calculate` trait declares four methods, which are defined in the `impl` block:

:::{literalinclude} templates/rs
:language: rust
:lines: 9-
:emphasize-lines: 2,6,9,12
:::

### Edit the `new` method

If the indicator is not configurable, then the `new` method and the struct (`R999`) can be left as-is.

If the indicator is configurable, then the `new` method reads the `settings` arguments and returns an instance of the struct (the capitalized `Self` token refers to the struct).

:::{hint}
To avoid unnecessary memory allocation, you can [`std::mem::take()`](https://doc.rust-lang.org/std/mem/fn.take.html) the `Settings` field named after the indicator. Indicators should not use other indicators' settings.
:::

::::{admonition} Example
:class: seealso

R999's methodology is "A competition completed with few submitted bids," with the default for "few" being 1 bid.

So far, you added the `R999` field to the `Settings` struct in `src/indicators/mod.rs`.

You can now move the field's value into the `R999` struct in the new module, `src/indicators/r999.rs`.

1. Add a corresponding field to the `R999` struct. All configurations are optional (in this case, `Option<usize>`), but the methodology is to set a default of 1. So, we can make the field non-optional on this struct:

   :::{code-block} rust
   :emphasize-lines: 3
   #[derive(Default)]
   pub struct R999 {
       threshold: usize,
   }
   :::

   If the field's default value couldn't be set at initialization, you would make it optional: for example, if the default value depended on order statistics, like quartiles.

   :::{code-block} rust
   :emphasize-lines: 3
   #[derive(Default)]
   pub struct R999 {
       threshold: Option<usize>,
   }
   :::

2. Move the value from the `Settings` struct into the `R999` struct:

   :::{code-block} rust
   :emphasize-lines: 3
       fn new(settings: &mut Settings) -> Self {
           Self {
               threshold: std::mem::take(&mut settings.R999).unwrap_or_default().threshold.unwrap_or(1),
           }
       }
   :::

   This incantation requires understanding the [`Option`](https://doc.rust-lang.org/std/option/index.html) type, the [`Default`](https://doc.rust-lang.org/std/default/trait.Default.html) trait and the [`std::mem::take()`](https://doc.rust-lang.org/std/mem/fn.take.html) function. In short, the `R999` struct's `threshold` field is set to the configured value if set and the default value (1), otherwise.

   If the field's default value couldn't be set at initialization, you would omit the `unwrap_or(1)`:

   :::{code-block} rust
   :emphasize-lines: 3
       fn new(settings: &mut Settings) -> Self {
           Self {
               threshold: std::mem::take(&mut settings.R999).unwrap_or_default().threshold,
           }
       }
   :::
::::

:::{admonition} Try it!
:class: tip

If you run the command again, the output should still be `{}`:

```bash
echo '{}' | cargo run -- indicators --settings settings.ini -
```
:::

### How data is prepared

As described in the [overall workflow](../../topics/workflow), data is prepared before it is processed. This avoids complicating the indicator calculations with many exceptions and edge cases.

Also, as described in the {ref}`prepare workflow<prepare-workflow>`, the `prepare` command should only warn about quality issues that it can fix and that interfere with the indicator calculations.

With that in mind, while you implement the indicator, think about whether:

- An existing {ref}`configuration<prepare-config>` of the `prepare` command should be edited to include additional fields.

  For example, at the time of writing, the `currency` property of the {ref}`defaults<fill-in-missing-values>` section only applies to `/bids/details[]/value/currency`, because no indicator uses other currency fields yet.

- A new configuration should be added, to address a quality issue you encountered.

[Create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues) to request any changes to the `prepare` command.

### How data is processed

Processing is divided into 3 steps: fold, reduce, and finalize. A trait method corresponds to each step.

Each method accepts an `item` argument, whose type is `Indicators` (named after the command).

The `Indicators` struct has a `results` field for the final results, and other fields – whose names are prefixed by indicator codes – for intermediate results:

:::{literalinclude} ../../../src/indicators/mod.rs
:language: rust
:start-at: struct Indicators
:end-at: "}"
:::

Cardinal processes compiled releases concurrently. The responsibilities of the 3 methods are:

Fold
: Operate on a single compiled release (its `release` argument), and write either final results or intermediate results.

Reduce
: Combine the intermediate results from the *fold* step (if any) into one `Indicators` instance. The `other` argument represents the instance that is to be combined.

Finalize
: Use the intermediate results to write final results.

Use the `set_result!` macro to write final results. It accepts an `item`, {ref}`group<indicators-demo>` (`OCID`, `Tenderer`, `Buyer`, or `ProcuringEntity`), identifier, indicator code, and result as a decimal (`f64`). For example:

```rust
set_result!(item, OCID, ocid, R999, 1.0);
```

Or:

```rust
set_result!(item, Buyer, id, R999, 1.0);
```

:::{hint}
If you remember, the indicator code was added as a variant to the `Indicator` enum in {ref}`indicators-boilerplate`.
:::

:::{note}
Implementing an indicator often raises questions about its methodology. In general, try to implement it such that its result is stable. In other words, new data can cause a red flag to be raised, but shouldn't cause it to be lowered. This typically means waiting for all relevant data to be available. For example, an indicator about the number of submitted bids should wait for all awards to be complete.
:::

### `fold` method

#### Final results

If the methodology considers compiled releases in isolation, the final results can be written by the `fold` method. In this case, the `reduce` and `finalize` methods can be deleted.

At this point, you need to know Rust, but you can study other indicators and adapt their code.

:::{admonition} Example
:class: seealso

R999's methodology is "A competition completed with few submitted bids." Comments are provided to ease reading.

``` rust
    fn fold(&self, item: &mut Indicators, release: &Map<String, Value>, ocid: &str) {
        // A competition is complete if an award is complete.

        // This verbose condition is a typical way to traverse JSON.
        if let Some(Value::Array(awards)) = release.get("awards")
            // There are one or more complete awards.
            && awards.iter().any(
                // An award is complete if its status is "active".
                |award| award.get("status").map_or(false, |status| status.as_str() == Some("active"))
            )
        {
            // The Indicators struct has methods for common operations.
            let bids = Indicators::get_submitted_bids(release).len();
            // Thresholds are typically interpreted as inclusive (<= or >=).
            if bids <= self.threshold {
                // The indicator's value is the number of submitted bids.
                set_result!(item, OCID, ocid, R999, bids as f64);
            }
        }
    }
```
:::

:::{admonition} Try it!
:class: tip

If you run:

```bash
echo '{"ocid":"F","bids":{"details":[{"status":"valid"}]},"awards":[{"status":"active"}]}' | cargo run -- indicators --settings settings.ini -
```

The compiled release should be flagged by the R999 indicator!

```none
{"OCID":{"F":{"R999":1.0}}}
```
:::

#### Intermediate results

If the methodology considers compiled releases in aggregate – for example, it uses order statistics to identify outliers – then the `fold` method writes intermediate results to new field(s) on the `Indicators` struct. For example:

```rust
    /// The documentation for the field.
    pub r999_variable_name: HashMap<String, Fraction>,
```

:::{admonition} To do
:class: caution
If you need guidance on this step, [create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues).
:::

### `reduce` method

:::{admonition} To do
:class: caution
If you need guidance on this step, [create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues).
:::

If the indicator considers and flags a subset of tenderers, buyers, or procuring entities, set `item.maps`. See `r038.rs`, for example.

### `finalize` method

:::{admonition} To do
:class: caution
If you need guidance on this step, [create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues).
:::

## Update the `init` command

- [ ] In `src/lib.rs`, edit the multiline string at the top of the `init` function to include a section for the new indicator, and any configurations as comments.
- [ ] In `docs/cli/init.md`, edit the command's output at the bottom of the file to match the multiline string.

:::{admonition} Next step
Now, you can {doc}`write the tests<test>`.
:::
