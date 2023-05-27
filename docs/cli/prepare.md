# prepare

The `prepare` command corrects quality issues within OCDS compiled releases.

Run the `help` command to read its description, output format and options:

```console
$ ocdscardinal help prepare
Correct quality issues within OCDS compiled releases in a line-delimited JSON file

Corrected data is written to standard output as line-delimited JSON.

Quality issues are written to standard error as CSV rows with the columns: line, ocid, path, array
indexes, incorrect value, error description.

Usage: ocdscardinal[EXE] prepare [OPTIONS] <FILE>

Arguments:
  <FILE>
          The path to the file (or "-" for standard input), in which each line is a contracting
          process as JSON text

Options:
  -s, --settings <SETTINGS>
          The path to the settings file

  -v, --verbose...
          Increase verbosity

  -h, --help
          Print help (see a summary with '-h')

```

## Workflow

:::{attention}
Before following this command's workflow, follow the earlier steps in the {doc}`../../topics/workflow`.
:::

1. Initialize a `settings.ini` file, using the {doc}`init` command:

   ```console
   $ ocdscardinal init settings.ini
   Settings written to "settings.ini".
   ```

1. Run the `prepare` command. For example, if your data is in `input.jsonl`, this command writes the corrected data to `prepared.jsonl` and the quality issues to `issues.csv`:

   ```bash
   ocdscardinal prepare --settings settings.ini input.jsonl > prepared.jsonl 2> issues.csv
   ```

1. Review the quality issues in the `issues.csv` file. Don't worry if many issues are reported: most are repetitive and can be fixed at once. Read the [demonstration](#demonstration) to learn how to interpret results.

1. Adjust the [configuration](#configuration) in the `settings.ini` file to fix the quality issues.

Repeat the last three steps until you are satisfied with the results.

:::{note}
This command is designed to only warn about quality issues (1) that it can fix and (2) that interfere with the calculation of {doc}`indicators/index`. If you want to check for other quality issues, contact OCP's [Data Support Team](mailto:data@open-contracting.org) about [Pelican](https://www.open-contracting.org/2020/01/28/meet-pelican-our-new-tool-for-assessing-the-quality-of-open-contracting-data/).
:::

## Demonstration

Corrected data is written to standard output. Quality issues are written to standard error.

Without redirection (`>`), standard output and standard error are both written to the console.

It is recommended to redirect standard output and standard error to separate files. For example:

```bash
ocdscardinal prepare --settings settings.ini input.jsonl > prepared.jsonl 2> issues.csv
```

::::{admonition} Example
:class: seealso

The bid status (`/bids/details[]/status`) is needed to determine whether a bid is {term}`submitted`, invited or withdrawn.

This simplified file contains a bid without a status:

:::{literalinclude} ../examples/prepare.jsonl
:language: json
:::

Without redirection, the `prepare` command writes both the quality issue and the (unchanged) data to the console:

```console
$ ocdscardinal prepare docs/examples/prepare.jsonl
1,"ocds-213czf-1",/bids/details[]/status,0,,not set
{"ocid":"ocds-213czf-1","bids":{"details":[{"id":1}]}}

```

Quality issues are reported as CSV rows. Adding a header and rendering the row as a table produces:

:::{csv-table}
:header: line,ocid,path,array indexes,incorrect value,error description

1,"ocds-213czf-1",/bids/details[]/status,0,,not set
:::

If you redirect the quality issues to a file, you can open the CSV as a spreadsheet.

::::

Given the context of this example, the columns can be used as follows.

:::{list-table}
:header-rows: 1

* - Column
  - Use
* - line
  - Find the problematic compiled release in the input file.
* - ocid
  - Find the problematic compiled release in another system, like the data source.
* - path
  - Consult the field that has an issue. This column can be used to sort and filter the issues.
* - array indexes
  - Find the problematic array entry in the compiled release. If the *path* contains multiple arrays (`[]`), the indexes are separated by periods.
* - incorrect value
  - Consult the value that caused the issue. If the issue is that the field isn't set, this is blank.
* - error description
  - Determine the potential solution to the issue. The possible values are:

    | Value | Meaning |
    | - | - |
    | not set | The field isn't set. To correct, [fill in missing values](#fill-in-missing-values). |
    | invalid | The code isn't valid. To correct, [re-map incorrect codes](#re-map-incorrect-codes). |
:::

## Configuration

### Normalize ID fields

Some ID fields allow both strings (`"1"`) and integers (`1`): for example, an award's `id` and a contract's `awardID`.
If the types are inconsistent, then lookups fail: for example, retrieving a contract's award or a supplier's address.

The command converts these ID fields to strings, in order to prevent this issue:

- `/buyer/id`
- `/tender/procuringEntity/id`
- `/bids/details[]/tenderers[]/id`
- `/awards[]/suppliers[]/id`

As new indicators are added, additional ID fields will be converted.

:::{note}
This behavior can't be disabled. If you need to disable it, [create an issue on GitHub](http://github.com/open-contracting/cardinal-rs/issues).
:::

### Fill in missing values

The command supports filling in:

- `/bids/details[]/value/currency`
- `/bids/details[]/items/classification/scheme`
- `/bids/details[]/status`
- `/awards[]/status`

To fill in one or more of these fields when the field isn't set, add a `[defaults]` section with relevant properties to your {doc}`../topics/settings`. For example:

```ini
[defaults]
currency = USD
item_classification_scheme = UNSPSC
bid_status = valid
award_status = active
```

As new indicators are added, additional currency and scheme fields will be filled in.

:::{tip}
Need to fill in other values? [Create an issue on GitHub](http://github.com/open-contracting/cardinal-rs/issues), or [email James McKinney](mailto:jmckinney@open-contracting.org), OCP's Head of Technology.
:::

### Re-map incorrect codes

The command supports substituting codes in these codelist fields:

- `/bids/details[]/status`, by adding a `[codelists.bidStatus]` section
- `/awards[]/status`, by adding a `[codelists.awardStatus]` section

To replace a code, add a property under the relevant section, in which the code to replace is the name, and its replacement is the value. For example:

```ini
[codelists.bidStatus]
Qualified = valid
Disqualified = disqualified
InTreatment = pending
```

:::{tip}
Need to re-map other values? [Create an issue on GitHub](http://github.com/open-contracting/cardinal-rs/issues), or [email James McKinney](mailto:jmckinney@open-contracting.org), OCP's Head of Technology.
:::
