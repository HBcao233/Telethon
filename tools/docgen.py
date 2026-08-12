"""
Run `sphinx-build` to create HTML documentation and detect errors.
"""

import subprocess
import sys


def run(*args: str) -> int:
    return subprocess.run((sys.executable, "-m", *args), check=False).returncode


def main() -> None:
    sys.exit(run("sphinx", "-n", "client/doc", "dist-doc"))


if __name__ == "__main__":
    main()
