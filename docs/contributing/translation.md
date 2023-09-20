# Translation

## Install dependencies

Install Sphinx, sphinx-intl and [Transifex CLI](https://developers.transifex.com/docs/cli):

```bash
pip install sphinx sphinx-intl
```

```bash
curl -o- https://raw.githubusercontent.com/transifex/cli/master/install.sh | bash
```

Create Transifex configuration:

```bash
sphinx-intl create-txconfig
```

## Extract strings to translate

```bash
sphinx-build -nW --keep-going -q -b gettext docs/ docs/_build/gettext
```

## Update Transifex configuration

```bash
sphinx-intl update-txconfig-resources \
    --pot-dir docs/_build/gettext \
    --locale-dir docs/locale \
    --transifex-organization-name open-contracting-partnership-1 \
    --transifex-project-name cardinal
```

## Push to Transifex

```bash
tx push -s
```

## Pull from Transifex

```bash
tx pull -f -a
```
