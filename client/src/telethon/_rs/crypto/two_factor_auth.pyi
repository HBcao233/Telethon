from typing import Buffer

def calculate_2fa(
    *,
    salt1: Buffer,
    salt2: Buffer,
    p: Buffer,
    g: int,
    g_b: Buffer,
    a: Buffer,
    password: Buffer,
) -> tuple[bytes, bytes]: ...
def check_p_and_g(p: Buffer, g: int) -> bool: ...
