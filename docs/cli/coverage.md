# coverage

The `coverage` command counts the number of times each JSON field is non-empty.

Run the `help` command to read its description, output format and options:

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

## Demonstration

```console
$ echo '{"phoneNumbers":[{"type": "home","number": "212 555-1234"},{"type": "office","number": "646 555-4567"}]}' | ocdscardinal coverage -
{"": 1, "/": 1, "/phoneNumbers": 1, "/phoneNumbers[]": 2, "/phoneNumbers[]/": 2, "/phoneNumbers[]/type": 2, "/phoneNumbers[]/number": 2}
```

## Caveats

```{note}
These edge cases are not expected to be encountered in real data.
```

If a member name is duplicated, only the last duplicate is considered.

```console
$ echo '{"a": 0, "a": null}' | ocdscardinal coverage -
{}
```

If a member name is empty, its path is the same as its parent object’s path:

```console
$ echo '{"": 0}' | ocdscardinal coverage -
{"": 1, "/": 2}
```

If a member name ends with `[]`, its path can be the same as a matching sibling’s path:

```console
$ echo '{"a[]": 0, "a": [0]}' | ocdscardinal coverage -
{"": 1, "/": 1, "/a": 1, "/a[]": 2}
```
