# Contributing

Pull requests are welcome and appreciated!

Please provide as much detail as possible on what you are trying to accomplish and any issues you are running into.

Also, please include any relevant code, error messages and example scripts with any changes.

## Setup

[uv](https://docs.astral.sh/uv/) is required to build async-siril.

## Testing

You can invoke the tests with the Makefile command:

```bash
make test
```

## Checking

Type checking and linting is done with `ruff` and `ty` and can be run with the Makefile command:

```bash
make check
```

## Formatting

You can also format the code with:

```bash
make format
```

## Releases

Releases can only be performed by the maintainers.

Please include a description of the changes in the changelog.
