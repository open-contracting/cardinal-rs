# Translation

## Install dependencies

1. Install Sphinx, sphinx-intl and [Crowdin CLI](https://support.crowdin.com/cli-tool/):

    ```bash
    pip install -r docs/requirements.txt sphinx-intl
    npm install -g @crowdin/cli
    ```

1. Set the Crowdin [personal access token](https://support.crowdin.com/enterprise/account-settings/#creating-a-personal-access-token) in the `.crowdin.yml` file in your home directory:

    ```yaml
    api_token: ...
    ```

## Extract strings to translate

```bash
sphinx-build -nW --keep-going -q -b gettext docs/ docs/_build/gettext
```

## Update Crowdin configuration

Using `manage.py` from [data-support](https://github.com/open-contracting/data-support/blob/main/manage.py):

```bash
manage.py update-crowdinyml-files
```

## Push to Crowdin

```bash
crowdin push sources
```

## Pull from Crowdin

```bash
crowdin pull translations
```
