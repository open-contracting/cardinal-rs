# OCDS Cardinal

Measure red flags and procurement indicators using OCDS data.

## Installation

The executable (a.k.a. binary or program) is `ocdscardinal`.

If you're a **macOS Homebrew** or **Linuxbrew** user, you can install ocdscardinal from our tap:

```bash
brew install open-contracting/tap/ocdscardinal
```

Otherwise, [ZIP archives of precompiled binaries for OCDS Cardinal are available for Windows, macOS and Linux.](http://github.com/open-contracting/cardinal-rs/releases)

If you're a **Rust programmer**, ocdscardinal can be installed with `cargo`:

```bash
cargo install ocdscardinal
```

## Commands

### coverage

```console
$ ocdscardinal help coverage
Count the number of times each field is non-empty in a line-delimited JSON file

The command walks the JSON tree, counting non-empty nodes. Empty nodes are "", [], {} and null, and
any nodes containing only empty nodes.

The result is a JSON object, in which keys are paths and values are counts.

The "" path corresponds to a line. A path ending with / corresponds to an object. A path ending with
[] corresponds to an array element. Other paths correspond to object members.

Usage: ocdscardinal[EXE] coverage [OPTIONS] <FILE>

Arguments:
  <FILE>
          The path to the file (or "-" for standard input), in which each line is JSON text

Options:
  -v, --verbose...
          Increase verbosity

  -h, --help
          Print help (see a summary with '-h')

```

For example:

```console
$ echo '{"phoneNumbers":[{"type": "home","number": "212 555-1234"},{"type": "office","number": "646 555-4567"}]}' | ocdscardinal coverage -
{"": 1, "/": 1, "/phoneNumbers": 1, "/phoneNumbers[]": 2, "/phoneNumbers[]/": 2, "/phoneNumbers[]/type": 2, "/phoneNumbers[]/number": 2}
```

#### Caveats

If a member name is duplicated, only the last duplicate is considered.

```console
$ echo '{"a": 0, "a": null}' | ocdscardinal coverage -
{}
```

If a member name is empty, its path is the same as its parent object's path:

```console
$ echo '{"": 0}' | ocdscardinal coverage -
{"": 1, "/": 2}
```

If a member name ends with `[]`, its path can be the same as a matching sibling's path:

```console
$ echo '{"a[]": 0, "a": [0]}' | ocdscardinal coverage -
{"": 1, "/": 1, "/a": 1, "/a[]": 2}
```

### indicators

```console
$ ocdscardinal help indicators
Calculate procurement indicators from OCDS compiled releases in a line-delimited JSON file

The result is a JSON object, in which keys are OCIDs and values are results.

Usage: ocdscardinal indicators [OPTIONS] <FILE>

Arguments:
  <FILE>
          The path to the file (or "-" for standard input), in which each line is a contracting
          process as JSON text

Options:
  -v, --verbose...
          Increase verbosity

  -h, --help
          Print help (see a summary with '-h')

```

For a given compiled release, an indicator is skipped if:

- The ``ocid`` isn't a string.
- The relevant fields aren't of the correct type. [#10](https://github.com/open-contracting/cardinal-rs/issues/10) [#13](https://github.com/open-contracting/cardinal-rs/issues/13)
- Monetary values, where relevant, use different currencies. [#11](https://github.com/open-contracting/cardinal-rs/issues/11)

#### NF024 The percentage difference between the winning bid and the second-lowest valid bid is an outlier

The difference for a compiled release is calculated as $x = {(secondLowestValidBidAmount - winningBidAmount) \over winningBidAmount}$. A compiled release is flagged if the difference is less than the lower fence – i.e. $x < Q_1 - 1.5(IQR)$, where $Q_1$ is the first quartile and $IQR$ is the interquartile range for the set of differences.

This indicator is skipped if:

- An award's status is pending or invalid.
- There are multiple active awards (winning bids).
- A bid is submitted by multiple suppliers. [#17](https://github.com/open-contracting/cardinal-rs/issues/17)
- An award is made to multiple suppliers. [#17](https://github.com/open-contracting/cardinal-rs/issues/17)
- The winning bid is not the lowest bid. (This indicator requires the award criteria to be price-only.)
