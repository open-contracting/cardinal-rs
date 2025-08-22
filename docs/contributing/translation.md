# Translation

## Install dependencies

1. Install Sphinx and sphinx-intl:

    ```bash
    pip install -r docs/requirements.txt sphinx-intl
    ```

## Extract strings to translate

```bash
sphinx-build -nW --keep-going -q -b gettext docs/ docs/_build/gettext
```

## Translate strings

See the [Software Development Handbook](https://ocp-software-handbook.readthedocs.io/en/latest/python/i18n.html).
