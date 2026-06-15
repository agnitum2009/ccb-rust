# LaTeX Production Plan

Date: 2026-06-10

## Target Layout

Default assumption until confirmed:

```text
docs/manuals/
  developer-guide/
    main.tex
    chapters/
    figures/
    tables/
    refs/
    Makefile
  user-guide/
    main.tex
    chapters/
    figures/
    tables/
    refs/
    Makefile
```

Use a small reproducible build surface. Avoid committing generated PDFs until
the user decides whether binary outputs belong in the repo.

## Source Style

- Keep prose in chapter files.
- Keep large generated tables in `tables/`.
- Keep diagrams in `figures/`, with source files when generated.
- Keep evidence citations in a simple bibliography or references appendix.
- Prefer stable source-path citations over line-number-heavy citations inside
  the LaTeX prose; line references can drift quickly.

## Build Candidates

Choose after checking installed TeX tooling:

```bash
latexmk -pdf main.tex
xelatex main.tex
```

If Chinese is the primary language, prefer XeLaTeX/LuaLaTeX with a CJK-capable
document class or package.

## Figures

Minimum developer-manual figures:

- high-level CCB control plane;
- project startup and supervision;
- windows/sidebar/topology materialization;
- ask communication flow;
- callback/artifact flow;
- provider home/session isolation;
- storage/state authority map;
- rolepack projection.

Figures should be regenerated from source diagrams where possible. If generated
bitmap/SVG assets are committed, keep source diagrams nearby.

## Review Gates

1. Skeleton build: table of contents, placeholder chapters, no broken includes.
2. Evidence build: source maps and artifact ledger included.
3. Complete draft: all chapters have prose, diagrams, and references.
4. Technical review: architecture and communication claims checked against
   source.
5. User-facing pass: examples and wording checked for clarity and safety.

