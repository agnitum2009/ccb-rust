# CCB Developer Guide

This directory contains the LaTeX source for the CCB developer manual.

Build:

```bash
make
```

Direct build command:

```bash
latexmk -xelatex -interaction=nonstopmode -halt-on-error main.tex
```

Output is written under `build/`.

Policy:

- Commit source, figures, tables, and build scripts.
- Do not commit generated PDFs by default.
- Primary prose is Chinese. Source identifiers, commands, config keys, and
  file paths remain in English.
