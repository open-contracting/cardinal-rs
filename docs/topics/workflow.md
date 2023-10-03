# Overall workflow

This page describes the general workflow when using Cardinal.

## 1. Collect data

Collect the data you want to analyze in OCDS format.

You can use the [OCP Data Registry](https://data.open-contracting.org) to download data from over 50 publishers. The Registry provides data as OCDS compiled releases in line-delimited JSON files (the same format expected by Cardinal).

:::{tip}
Is the data you're interested in not in OCDS format? Contact OCP's [Data Support Team](mailto:data@open-contracting.org) to see how we can help.
:::

## 2. Prepare data

### Format

If you are *not* using data from the Registry, ensure that the [releases or records](https://standard.open-contracting.org/latest/en/primer/releases_and_records/) are merged into compiled releases, and that the compiled releases are upgraded to OCDS 1.1 (the version since 2017).

You can use [OCDS Kit](https://ocdskit.readthedocs.io/en/latest/)'s command-line interface to [compile](https://ocdskit.readthedocs.io/en/latest/cli/ocds.html#compile) and [upgrade](https://ocdskit.readthedocs.io/en/latest/cli/ocds.html#upgrade) the OCDS data.

### Quality

For the indicator results to be reliable, the input data must be good quality.

You can use the {doc}`../cli/prepare` command to identify and correct quality issues.

## 3. Explore data

To inform your {ref}`selection and configuration<indicators-workflow>` of indicators, you can explore your data using JSON processors like [jaq](https://github.com/01mf02/jaq) (faster) or [jq](https://stedolan.github.io/jq/) (slower).

For example, if the publisher uses `/tender/procurementMethodDetails` for the local name of the procurement method, you can count the occurrences of each procurement method with:

```console
$ jaq 'reduce (inputs | .tender.procurementMethodDetails) as $s ({}; .[$s] += 1)' input.jsonl
{
  "Compras por Debajo del Umbral": 58958,
  "Comparacion de Precios": 4837,
  "Compras Menores": 29175,
  "Procesos de Excepcion": 4629,
  "Licitacion Publica Nacional": 1258,
  "Sorteo de Obras": 29,
  "Licitacion Publica Internacional": 29,
  "Subasta Inversa": 40,
  "Licitacion Restringida": 5
}
```

If the publisher uses a classification system for products and services, like UNSPSC or CPV, you can count the occurrences of each segment/division of the classification with:

```console
$ jaq 'reduce (inputs | .awards[]?.items[]?.classification.id | values | tostring | .[:2]) as $s ({}; .[$s] += 1)' input.jsonl
{
  "42": 26933,
  "43": 12549,
  "81": 2805,
  ...
}
```

## 4. Calculate indicators

Use the {doc}`../cli/indicators/index` command to calculate procurement indicators and red flags.

Additional information on this step is provided in the [command's documentation](../cli/indicators/index).

## 5. Analyze results

:::{admonition} Coming soon
:class: important
The Open Contracting Partnership is building business intelligence tools, using the indicator results from Cardinal.

Are you interested? Contact OCP's [Data Support Team](mailto:data@open-contracting.org).
:::
