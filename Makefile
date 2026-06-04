venv:
	python -m venv .venv

codegen: venv
	.venv/bin/pip install -e generator/
	.venv/bin/python tools/codegen.py
