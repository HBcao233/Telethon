VENV = .venv

$(VENV):
	python -m venv $(VENV)

codegen: $(VENV)
	.venv/bin/pip install -e generator/
	.venv/bin/python tools/codegen.py

dev: $(VENV)
	maturin develop
