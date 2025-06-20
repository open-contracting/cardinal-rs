# prepare

The `prepare` command corrects quality issues within OCDS compiled releases.

Run the `help` command to read its description, output format and options:

```console
$ ocdscardinal help prepare
Correct quality issues within OCDS compiled releases in a line-delimited JSON file

Corrected data is written to standard output as line-delimited JSON.

Quality issues are written to standard error as CSV rows with the columns: line, ocid, path, array
indexes, incorrect value, error description.

Usage: ocdscardinal[EXE] prepare [OPTIONS] --output <OUTPUT> --errors <ERRORS> <FILE>

Arguments:
  <FILE>
          The path to the file (or "-" for standard input), in which each line is a contracting
          process as JSON text

Options:
  -s, --settings <SETTINGS>
          The path to the settings file

  -v, --verbose...
          Increase verbosity

  -o, --output <OUTPUT>
          The file to which to write corrected data (or "-" for standard output)

  -e, --errors <ERRORS>
          The file to which to write quality issues (or "-" for standard output)

  -h, --help
          Print help (see a summary with '-h')

```

(prepare-workflow)=
## Workflow

:::{attention}
Before following this command's workflow, follow the earlier steps in the {doc}`../../topics/workflow`.
:::

1. Initialize a `settings.ini` file, using the {doc}`init` command:

   ```console
   $ ocdscardinal init settings.ini
   Settings written to settings.ini.
   ```

1. Run the `prepare` command. For example, if your data is in `input.jsonl`, this command writes the corrected data to `prepared.jsonl` and the quality issues to `issues.csv`:

   ```bash
   ocdscardinal prepare --settings settings.ini --output prepared.jsonl --errors issues.csv input.jsonl
   ```

1. Review the quality issues in the `issues.csv` file. Don't worry if many issues are reported: most are repetitive and can be fixed at once. Read the [demonstration](#demonstration) to learn how to interpret results.

1. Adjust the [configuration](#configuration) in the `settings.ini` file to fix the quality issues.

Repeat the last three steps until you are satisfied with the results.

:::{note}
This command is designed to only warn about quality issues (1) that it can fix and (2) that interfere with the calculation of {doc}`indicators/index`. If you want to check for other quality issues, contact OCP's [Data Support Team](mailto:data@open-contracting.org) about [Pelican](https://www.open-contracting.org/2020/01/28/meet-pelican-our-new-tool-for-assessing-the-quality-of-open-contracting-data/).
:::

## Demonstration

::::{admonition} Example
:class: seealso

The bid status (`/bids/details[]/status`) is needed to determine whether a bid is {term}`submitted`, invited or withdrawn.

This simplified file contains a bid without a status:

:::{literalinclude} ../examples/prepare.jsonl
:language: json
:::

For this demonstration, write the quality issues to the console:

```console
$ ocdscardinal prepare --output prepared.jsonl --errors - docs/examples/prepare.jsonl
1,ocds-213czf-1,/bids/details[]/status,0,,not set

```

Quality issues are reported as CSV rows. Adding a header and rendering the row as a table produces:

:::{csv-table}
:header: line,ocid,path,array indexes,incorrect value,error description

1,"ocds-213czf-1",/bids/details[]/status,0,,not set
:::

If you write the quality issues to a file instead of the console, you can open the CSV as a spreadsheet.

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
    | invalid | The code isn't valid. To correct, [re-map incorrect codes](#re-map-invalid-codes). |
    | is zero | The bid's value is zero. To correct, [redact incorrect values](#redact-incorrect-values). |
:::

This command logs a warning if a JSON text isn't valid or isn't an object.

(prepare-config)=
## Configuration

For each configuration, additional fields will be supported as new indicators are added.

### Correct structural errors

If a value is an object where OCDS expects an array, then calculations fail.

The command replaces each such object with an array containing the object. The command supports replacing:

- `/bids/details[]/tenderers`
- `/awards/suppliers`

:::{note}
This behavior can't be disabled. If you need to disable it, [create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues).
:::

### Normalize ID fields

Some ID fields allow both strings (`"1"`) and integers (`1`): for example, an award's `id` and a contract's `awardID`.
If the types are inconsistent, then lookups fail: for example, retrieving a contract's award or a supplier's address.

The command converts these ID fields to strings, in order to prevent this issue:

- `/parties[]/id`
- `/buyer/id`
- `/tender/procuringEntity/id`
- `/bids/details[]/tenderers[]/id`
- `/awards[]/id`
- `/awards[]/suppliers[]/id`
- `/awards[]/items[]/classification/id`
- `/contracts[]/awardID`

:::{note}
This behavior can't be disabled. If you need to disable it, [create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues).
:::

(fill-in-missing-values)=
### Fill in missing values

The command supports filling in:

- `/bids/details[]/value/currency`
- `/bids/details[]/items[]/classification/scheme`
- `/bids/details[]/status`
- `/awards[]/items[]/classification/scheme`
- `/awards[]/status`
- `/parties[]/roles[]`

To fill in one or more of these fields when the field isn't set, add a `[defaults]` section with relevant properties to your {doc}`../topics/settings`. For example:

```ini
[defaults]
currency = USD
item_classification_scheme = UNSPSC
bid_status = valid
award_status = active
party_roles = true
```

Every organization reference (like `/buyer/id`) should have a corresponding value (like 'buyer') in the `/parties[]/roles[]` array. If the corresponding value is missing, set `party_roles = true`. This supports:

- `/buyer/id` for the 'buyer' role
- `/tender/procuringEntity/id` for the 'procuringEntity' role
- `/bids/details[]/tenderers[]/id` for the 'tenderer' role
- `/awards[]/suppliers[]/id` for the 'supplier' role

:::{tip}
Need to fill in other values? [Create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues), or [email James McKinney](mailto:jmckinney@open-contracting.org), OCP's Head of Technology.
:::

(redact-incorrect-values)=
### Redact incorrect values

:::{tip}
Need to redact other values? [Create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues), or [email James McKinney](mailto:jmckinney@open-contracting.org), OCP's Head of Technology.
:::

#### Monetary amounts

Indicators assume that amount values are accurate. If an amount field is assigned a placeholder value, this assumption fails. For example, if 0 is used when the amount is confidential or wasn't entered, then the lowest bids might be miscalculated.

To redact an amount value, add a `[redactions]` section with an `amount` property to your {doc}`../topics/settings`. Its value is a pipe-separated list. For example:

```ini
[redactions]
amount = 0|99999999
```

This configuration supports redacting values from:

- `/bids/details[]/value/amount`

#### Organization IDs

Indicators assume that ID values represent distinct entities. If an ID field is assigned a placeholder value, this assumption fails. For example, if the placeholder value is used frequently, then the top suppliers might be miscalculated.

To redact an ID value from an organization reference, add a `[redactions]` section with an `organization_id` property to your {doc}`../topics/settings`. Its value is a pipe-separated list. For example:

```ini
[redactions]
organization_id = my-placeholder|dummy-value
```

This configuration supports redacting values from:

- `/parties[]/id`
- `/buyer/id`
- `/tender/procuringEntity/id`
- `/bids/details[]/tenderers[]/id`
- `/awards[]/suppliers[]/id`

### Re-map invalid codes

The command supports substituting codes in these codelist fields:

- `/bids/details[]/status`, by adding a `[codelists.bid_status]` section
- `/awards[]/status`, by adding a `[codelists.award_status]` section

To replace a code, add a property under the relevant section, in which the code to replace is the name, and its replacement is the value. For example:

```ini
[codelists.bid_status]
Qualified = valid
Disqualified = disqualified
InTreatment = pending
```

:::{tip}
Need to re-map other values? [Create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues), or [email James McKinney](mailto:jmckinney@open-contracting.org), OCP's Head of Technology.
:::

### Move auction bids

Reverse auctions are under [discussion](https://github.com/open-contracting/standard/issues/904) for inclusion in OCDS. Some publishers model auction bids at the non-standard `/auctions[]/stages[]/bids[]` instead of at the standard `/bids/details[]`.

To move auction bids to the standard location, add a `[modifications]` section with a `move_auctions` property to your {doc}`../topics/settings`. For example:

```ini
[modifications]
move_auctions = true
```

If enabled, this configuration logs a warning if both `/auctions` and `/bids` are present.

### Prefix organization IDs

If the `id` field of an organization reference (like `/buyer/id`) doesn't match the `id` field of a `/parties[]` entry, then lookups fail. For example, `/parties[]/id` might include the identifier scheme (like "DO-RPE-1422"), but `/bids/details[]/tenderers[]/id` might use the identifier alone (like "1422").

To prefix text to the `id` field of an organization reference, add a `[modifications]` section with `prefix_buyer_or_procuring_entity_id` and/or `prefix_tenderer_or_supplier_id` properties to your {doc}`../topics/settings`. For example:

```ini
[modifications]
prefix_buyer_or_procuring_entity_id = DO-UC-
prefix_tenderer_or_supplier_id = DO-RPE-
```

These configurations support prefixing text to:

- `/buyer/id`
- `/tender/procuringEntity/id`
- `/bids/details[]/tenderers[]/id`
- `/awards[]/suppliers[]/id`

Text isn't prefixed if the `id` field is [redacted](#redact-incorrect-values) or if it starts with the text.

### Standardize unconstrained values

Text fields with non-standardized values can be standardized to ease the configuration of {doc}`indicators<indicators/index>`. For example, if a value is formatted as `{mutual category} - {individual detail}`, you can split the value on the `-` separator and keep the `{mutual category}` prefix.

To standardize a value by splitting it on a separator and keeping the prefix, add a `[modifications]` section with a `split_procurement_method_details` property to your {doc}`../topics/settings`. For example:

```ini
[modifications]
split_procurement_method_details = -
```

This configuration supports standardizing values in:

- `/tender/procurementMethodDetails`

:::{tip}
Need to standardize other values? [Create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues), or [email James McKinney](mailto:jmckinney@open-contracting.org), OCP's Head of Technology.
:::

### Replace incorrect award statuses

In rare cases, it is appropriate to change an award's status according to its contracts' statuses.

:::{admonition} Example
:class: seealso

The Government of Ruritania bundles many decisions into one award object, and uses the contract object as a proxy for the individual decision. As such, every award object is related to one or more contract objects. If the individual decision is cancelled (for example, the award is appealed at court or the supplier refuses to sign the contract), the contract object's status is changed to cancelled. The award object's status remains active.
:::

Indicators assume that awards, not contracts, represent individual decisions – in conformance with OCDS. In the example, to better satisfy this assumption, the status of an award can be changed to cancelled if the status of every related contract is cancelled.

To replace an award's status in this way, add a `[corrections]` section with a `award_status_by_contract_status` property to your {doc}`../topics/settings`. Its value is a boolean. For example:

```ini
[corrections]
award_status_by_contract_status = true
```

:::{tip}
Need to correct other values? [Create an issue on GitHub](https://github.com/open-contracting/cardinal-rs/issues), or [email James McKinney](mailto:jmckinney@open-contracting.org), OCP's Head of Technology.
:::
