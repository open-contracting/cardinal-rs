# Add documentation

## Document the indicator

Open the documentation page (`docs/cli/indicators/R/999.md`, in this example) in a text editor.

Title
: Use at most five words to name the indicator. This title appears in the documentation's sidebar and is adopted by other tools, like business intelligence charts.

Description
: Write a concise, one-line sentence communicating the condition under which the red flag is raised. Read {ref}`other titles<list>` to match the word choice.

Methodology
: - Read {ref}`other methodologies<list>`, to match the word choice, sentence construction, and use of bullet lists and paragraph breaks.
  - Bold the first occurrence of each word in the methodology that can be configured (*Configuration*) or interpreted (*Output*).
  - Write a brief example containing names and numbers, to ease the interpretation of the methodology.
  - Explain why the methodology indicates a red flag.

Output
: Bold the word for the concept from the methodology to which the output corresponds. Declare the type of the output if it is ambiguous.

Configuration
: Read {ref}`other configurations<list>`, to match the phrasing and styling. Bold the words for the concepts from the methodology that are configurable. Declare the type of the configuration.

  If there is no configuration, delete this section.

(indicators-exclusions)=
Exclusions
: Describe any features that cause the indicator to skip a compiled release, as a bullet list.

  If there are no exclusions, delete this section.

Assumptions
: Mention any assumptions made by the methodology. (In general, there should be none.)

  If there are no assumptions, delete this section.

Demonstration
: Adapt your test files to update the `docs/examples/R/999.jsonl` file with a minimal input, and the `console` code block with the corresponding output. Run `cargo test` to check the output.

## Update the list of indicators and the changelog

- [ ] In `docs/cli/indicators/index.md`, add the indicator in alphabetical order to the relevant table:

  ```md
  * - [R999](R/999)
    - [The title of the indicator](R/999)
    - The description of the indicator.
  ```

- [ ] In `docs/changelog.md`, add a changelog entry for the indicator:

  ```md
  ### Added

  - {doc}`cli/indicators/index` command:
    - R999 (*The title of the indicator*).
  ```

**You're done!** 🎉 We welcome all new indicators as pull requests on [GitHub](https://github.com/open-contracting/cardinal-rs).
