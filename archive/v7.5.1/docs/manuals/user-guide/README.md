# CCB User Guide

This directory contains the LaTeX source for the CCB user manual.

Build:

```bash
make
```

Direct build:

```bash
latexmk -xelatex -interaction=nonstopmode -halt-on-error main.tex
```

Output is written under `build/`.

Primary prose is Chinese. Commands, config keys, and source identifiers remain
in English.
