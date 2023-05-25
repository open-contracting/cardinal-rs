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
  -v, --verbose...  Increase verbosity
  -h, --help        Print help

```

```{seealso}
An introduction to the {doc}`../topics/settings`.
```

## Demonstration

Write a settings file to `settings.ini`:

```console
$ ocdscardinal init settings.ini
Settings written to "settings.ini".

```

```{warning}
If `settings.ini` already exists, its contents are overwritten.
```

Preview the content of the settings file:

```console
$ ocdscardinal init -
; `prepare` command
;
; Read the documentation at:
; https://cardinal.readthedocs.io/en/latest/cli/prepare.html

[defaults]
; currency = USD
; item_classification_scheme = UNSPSC
; bid_status = valid
; award_status = active

[codelists.bidStatus]
; qualified = valid

; `indicators` command
;
; Read the documentation at:
; https://cardinal.readthedocs.io/en/latest/cli/indicators/

[R024]
; threshold = 0.05

[R025]
; percentile = 75
; threshold = 0.05

[R035]
; threshold = 1

[R036]

[R038]
; threshold = 0.5

```
