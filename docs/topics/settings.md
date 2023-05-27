# Settings file

A settings file can be used to configure the behavior of the {doc}`../cli/prepare` and {doc}`../cli/indicators/index` commands, using the `--settings` option to provide the path to the settings file.

## File format

The settings file is in INI format (don't worry – it's simple).

The file is split into sections. A section starts with a name in square brackets, like this:

```ini
[R035]
```

A section can contain zero or more properties, like this:

```ini
[R035]
threshold = 1
```

A property is a name and a value, with an equals sign (=) in between.

You can document your configuration by starting a line with a semi-colon (;), like this:

```ini
[R035]
; Increase the threshold to reduce the number of false positives.
threshold = 3
```

These lines are known as *comments*. (You can also use a number sign (#) instead of a semi-colon.)
