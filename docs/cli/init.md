# init

The `init` command writes a default settings file for configuration.

Run the `help` command to read its description:

```console
$ ocdscardinal help init
Write a default settings file for configuration

Usage: ocdscardinal[EXE] init [OPTIONS] <FILE>

Arguments:
  <FILE>  The path to the settings file to write

Options:
  -f, --force       Overwrite the settings file if it already exists
  -v, --verbose...  Increase verbosity
  -h, --help        Print help

```

:::{seealso}
An introduction to the {doc}`../topics/settings`.
:::

## Demonstration

Write a settings file to `settings.ini`:

```console
$ ocdscardinal init settings.ini
Settings written to "settings.ini".

```

Preview the content of the settings file:

```console
$ ocdscardinal init -
; currency = USD
; fixed_price_procurement_methods = Random Selection

; `prepare` command
;
; Read the documentation at:
; https://cardinal.readthedocs.io/en/latest/cli/prepare.html

[defaults]
; currency = USD
; item_classification_scheme = UNSPSC
; bid_status = valid
; award_status = active

[codelists.BidStatus]
; qualified = valid

[codelists.AwardStatus]
; Active = active

; `indicators` command
;
; Read the documentation at:
; https://cardinal.readthedocs.io/en/latest/cli/indicators/

[R024]
; threshold = 0.05

[R025]
; percentile = 75
; threshold = 0.05

[R028]

[R030]

[R035]
; threshold = 1

[R036]

[R038]
; threshold = 0.5
; minimum_submitted_bids = 2
; minimum_contracting_processes = 2

[R048]
; digits = 2
; threshold = 10

[R058]
; threshold = 0.5

```
