# Changelog

## 0.0.3 (Unreleased)

### Added

- {doc}`cli/init` command.
- {doc}`cli/prepare` command.
- {doc}`cli/indicators/index` command:

  - R025 (*The ratio of winning bids to submitted bids for a top tenderer is a low outlier*).
  - R036 (*The lowest submitted bid is disqualified, while the award criterion is price only*).
  - R038 (*The ratio of disqualified bids to submitted bids is a high outlier per buyer, procuring entity or tenderer*).

- Expand documentation.

### Changed

- {doc}`cli/indicators/index` command:

  - Split indicators into trait objects.
  - Rename indicators from `NF###` to `R###`.

### Fixed

- Commands no longer error on `SIGPIPE` signal.

## 0.0.2 (2023-02-13)

### Added

- {doc}`cli/indicators/index` command:

  - R035 (*Bids are disqualified if not submitted by the single tenderer of the winning bid*).
  - Add `--settings SETTINGS` option for configuration file.
  - Add documentation.

## 0.0.1 (2023-02-13)

First release, including:

- R024 (*The percentage difference between the winning bid and the second-lowest valid bid is a low outlier*).
