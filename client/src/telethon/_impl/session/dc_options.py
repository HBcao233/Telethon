from .types import DcOption

DEFAULT_DC = 2

KNOWN_DC_OPTIONS = {
    1: DcOption(1, "149.154.175.53:443", None, None),
    2: DcOption(2, "149.154.167.41:443", None, None),
    3: DcOption(3, "149.154.175.100:443", None, None),
    4: DcOption(4, "149.154.167.92:443", None, None),
    5: DcOption(5, "91.108.56.104:443", None, None),
}

TEST_DC_OPTIONS = {
    1: DcOption(1, "149.154.175.10:443", "[2001:b28:f23d:f001::e]:443", None),
    2: DcOption(2, "149.154.167.40:443", "[2001:67c:4e8:f002::e]:443", None),
    3: DcOption(3, "149.154.175.117:443", "[2001:b28:f23d:f003::e]:443", None),
}
