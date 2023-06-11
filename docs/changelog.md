# Changelog

## 0.0.5 (Unreleased)

### Added

- {doc}`cli/init` command:
  - Add `--force` (`-f`) option to overwrite an existing file.
- {doc}`cli/indicators/index` command:
  - Add `--maps` option to include the `Maps` key.
  - Add `--no-meta` option to omit the `Meta` key.
  - [R038](cli/indicators/R/038): Add `minimum_submitted_bids` and `minimum_contracting_processes` configurations.

### Changed

- Prefix an error about an unknown property in the settings file with the path at which it occurred.
- {doc}`cli/init` command:
  - Add `currency` property to default file.
- {doc}`cli/prepare` command:
  - Improve write performance.
- {doc}`cli/indicators/index` command:
  - Add `Meta` key to assist interpretation of results.
  - The `--count` option writes to standard error instead of standard output, to not mix outputs.
  - All `threshold` and `percentile` configurations are consistently interpreted as inclusive.

## 0.0.4 (2023-05-30)

### Changed

- {doc}`cli/prepare` command:
  - Add `--output` (`-o`) and `--errors` (`-e`) options, instead of using shell redirection.
  - Fill in `/awards[]/items[]/classification/scheme` with `item_classification_scheme`.

## 0.0.3 (2023-05-29)

### Added

- {doc}`cli/init` command.
- {doc}`cli/prepare` command.
- {doc}`cli/indicators/index` command:
  - [R025](cli/indicators/R/025) (*The ratio of winning bids to submitted bids for a top tenderer is a low outlier*).
  - [R036](cli/indicators/R/036) (*The lowest submitted bid is disqualified, while the award criterion is price only*).
  - [R038](cli/indicators/R/038) (*The ratio of disqualified bids to submitted bids is a high outlier per buyer, procuring entity or tenderer*).
- Expand documentation.

### Changed

- Error on unknown configurations in the settings file.
- {doc}`cli/indicators/index` command:
  - [Enable](cli/indicators/index.md#enable-an-indicator) indicators in the settings file.
  - Rename indicators from `NF###` to `R###`.
  - Remove "OCID" from output if no OCIDs reported.
  - Split indicators into trait objects.
  - Preserve top-level key order in the JSON output.
- {doc}`cli/coverage` command:
  - Preserve JSON path key order in the JSON output.

### Fixed

- Commands no longer error on `SIGPIPE` signal.

## 0.0.2 (2023-02-13)

### Added

- {doc}`cli/indicators/index` command:
  - [R035](cli/indicators/R/035) (*Bids are disqualified if not submitted by the single tenderer of the winning bid*).
  - Add `--settings SETTINGS` option for the settings file.
  - Add documentation.

## 0.0.1 (2023-02-13)

First release, including:

- [R024](cli/indicators/R/024) (*The percentage difference between the winning bid and the second-lowest valid bid is a low outlier*).
