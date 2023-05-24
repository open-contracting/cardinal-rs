# Add an indicator

Indicators are coded: for example, `R001`. If the indicator has no code in the [resources](https://www.open-contracting.org/resources/) of the Open Contracting Partnership, [create an issue on GitHub](http://github.com/open-contracting/cardinal-rs/issues) to reserve a code.

In this tutorial, the example indicator is given the code `R999`.

## Add boilerplate

- Create empty files for the …:

  Module
  : `src/indicators/r999.rs`

  Test input
  : `tests/fixtures/indicators/R999.jsonl`

  Test output
  : `tests/fixtures/indicators/R999.expected`

  Documentation
  : `docs/cli/indicators/R/999.md`

- In `src/indicators/mod.rs`:

  1. Declare the new module in alphabetical order at the top of the file:

     ```rust
     pub mod r999;
     ```

  1. Add a field to the `Settings` struct. Re-use other structs, if possible. If the indicator accepts no options, or you don't know, use a `HashMap`.

     ```rust
         pub R999: Option<HashMap<String, String>>,
     ```

  1. Add a variant to the `Indicator` enum. The variants are keys in the JSON output of the {doc}`../cli/indicators/index` command:

     ```rust
         R999,
     ```

  1. Add (and document) any fields to the `Indicators` struct. The fields are used to merge information extracted from different contracting processes:

     ```rust
         /// The documentation for the field.
         pub r999_variable_name: HashMap<String, Fraction>,
     ```

- In `src/lib.rs`:

  1. Import the new struct in alphabetical order at the top of the file:

     ```rust
     use crate::indicators::r999::R999;
     ```
  1. Add the new struct in alphabetical order to the ``add_indicators!`` macro call:

     ```rust
     add_indicators!(indicators, settings, ..., R999);
     ```

- In `docs/cli/indicators/index.md`:

  1. Add the new indicator in alphabetical order to the relevant table:

     ```md
     * - [R999](R/999)
       - [The title of the indicator](R/999)
     ```
