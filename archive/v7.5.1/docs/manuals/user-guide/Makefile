LATEXMK ?= latexmk
MAIN ?= main.tex

.PHONY: all clean distclean

all:
	$(LATEXMK) -xelatex -interaction=nonstopmode -halt-on-error -outdir=build $(MAIN)

clean:
	$(LATEXMK) -C -outdir=build $(MAIN)

distclean: clean
	rm -rf build
