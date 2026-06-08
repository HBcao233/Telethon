VENV = .venv

$(VENV):
	python -m venv $(VENV)

codegen: $(VENV)
	.venv/bin/pip install -e generator/
	.venv/bin/python tools/codegen.py

dev: $(VENV)
	maturin develop

check:
	maturin develop --extras dev
	.venv/bin/python tools/check.py

docgen: $(VENV)
	maturin develop --extras doc
	.venv/bin/python tools/docgen.py
