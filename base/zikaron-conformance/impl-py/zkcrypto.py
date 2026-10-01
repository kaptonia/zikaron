"""secp256k1, EIP-191 digests, RFC 6979 signing and public-key recovery.

Pure Python point arithmetic (Jacobian coordinates); no secp256k1 library.
"""

import hashlib
import hmac


P = 2 ** 256 - 2 ** 32 - 977
N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
HALF_N = (N - 1) // 2
GX = 0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
GY = 0x483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8

INF = (0, 0, 0)


_KECCAK_RC = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
    0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
    0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
]
_KECCAK_ROT = [
    [0, 36, 3, 41, 18], [1, 44, 10, 45, 2], [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56], [27, 20, 39, 8, 14],
]
_M64 = (1 << 64) - 1


def _keccak_f1600(lanes):
    for rc in _KECCAK_RC:
        c = [lanes[x][0] ^ lanes[x][1] ^ lanes[x][2] ^ lanes[x][3] ^ lanes[x][4] for x in range(5)]
        d = [c[(x - 1) % 5] ^ (((c[(x + 1) % 5] << 1) | (c[(x + 1) % 5] >> 63)) & _M64) for x in range(5)]
        lanes = [[lanes[x][y] ^ d[x] for y in range(5)] for x in range(5)]
        b = [[0] * 5 for _ in range(5)]
        for x in range(5):
            for y in range(5):
                r = _KECCAK_ROT[x][y]
                v = lanes[x][y]
                b[y][(2 * x + 3 * y) % 5] = ((v << r) | (v >> (64 - r))) & _M64 if r else v
        lanes = [[b[x][y] ^ ((~b[(x + 1) % 5][y]) & b[(x + 2) % 5][y]) for y in range(5)] for x in range(5)]
        lanes[0][0] ^= rc
    return lanes


def keccak256(data):
    """Keccak-256 (the pre-FIPS padding 0x01, as Ethereum uses), rate 136 bytes."""
    rate = 136
    data = bytes(data)
    padded = bytearray(data)
    padded.append(0x01)
    while len(padded) % rate:
        padded.append(0x00)
    padded[-1] |= 0x80
    lanes = [[0] * 5 for _ in range(5)]
    for off in range(0, len(padded), rate):
        block = padded[off:off + rate]
        for k in range(rate // 8):
            lane = int.from_bytes(block[8 * k:8 * k + 8], "little")
            lanes[k % 5][k // 5] ^= lane
        lanes = _keccak_f1600(lanes)
    out = bytearray()
    for k in range(4):
        out += lanes[k % 5][k // 5].to_bytes(8, "little")
    return bytes(out)


def sha256(data):
    return hashlib.sha256(data).digest()


# --------------------------------------------------------------------------
# Jacobian point arithmetic on y^2 = x^3 + 7
# --------------------------------------------------------------------------

def jdouble(p):
    X1, Y1, Z1 = p
    if Z1 == 0 or Y1 == 0:
        return INF
    A = (Y1 * Y1) % P
    B = (4 * X1 * A) % P
    C = (8 * A * A) % P
    D = (3 * X1 * X1) % P
    X3 = (D * D - 2 * B) % P
    Y3 = (D * (B - X3) - C) % P
    Z3 = (2 * Y1 * Z1) % P
    return (X3, Y3, Z3)


def jadd(p1, p2):
    X1, Y1, Z1 = p1
    X2, Y2, Z2 = p2
    if Z1 == 0:
        return p2
    if Z2 == 0:
        return p1
    Z1Z1 = (Z1 * Z1) % P
    Z2Z2 = (Z2 * Z2) % P
    U1 = (X1 * Z2Z2) % P
    U2 = (X2 * Z1Z1) % P
    S1 = (Y1 * Z2 * Z2Z2) % P
    S2 = (Y2 * Z1 * Z1Z1) % P
    if U1 == U2:
        if S1 != S2:
            return INF
        return jdouble(p1)
    H = (U2 - U1) % P
    R = (S2 - S1) % P
    HH = (H * H) % P
    HHH = (H * HH) % P
    V = (U1 * HH) % P
    X3 = (R * R - HHH - 2 * V) % P
    Y3 = (R * (V - X3) - S1 * HHH) % P
    Z3 = (Z1 * Z2 * H) % P
    return (X3, Y3, Z3)


def jneg(p):
    X, Y, Z = p
    if Z == 0:
        return p
    return (X, (P - Y) % P, Z)


def jmul(k, p):
    """Fixed 4-bit window scalar multiplication."""
    k %= N
    if k == 0 or p[2] == 0:
        return INF
    tbl = [INF, p]
    for i in range(2, 16):
        tbl.append(jadd(tbl[i - 1], p))
    r = INF
    i = ((k.bit_length() + 3) // 4) * 4 - 4
    while i >= 0:
        r = jdouble(jdouble(jdouble(jdouble(r))))
        d = (k >> i) & 15
        if d:
            r = jadd(r, tbl[d])
        i -= 4
    return r


def to_affine(p):
    X, Y, Z = p
    if Z == 0:
        return None
    zi = pow(Z, P - 2, P)
    zi2 = (zi * zi) % P
    return ((X * zi2) % P, (Y * zi2 % P * zi) % P)


G = (GX, GY, 1)


def pubkey(priv):
    return to_affine(jmul(priv, G))


def address_of(pt):
    x, y = pt
    return keccak256(x.to_bytes(32, "big") + y.to_bytes(32, "big"))[-20:]


# --------------------------------------------------------------------------
# s5.3 EIP-191 digest
# --------------------------------------------------------------------------

_PREFIX = b"\x19" + b"Ethereum Signed Message:" + b"\x0a"


def eip191_digest(msg):
    return keccak256(_PREFIX + str(len(msg)).encode("ascii") + msg)


def message(domain, presig32):
    """s5.2 / s6.6: D || 0x0A || hex32(presig)."""
    return domain.encode("utf-8") + b"\x0a" + b"0x" + presig32.hex().encode("ascii")


# --------------------------------------------------------------------------
# s5.4 recovery
# --------------------------------------------------------------------------

def recover(digest32, r, s, i):
    """Return the recovered public key as an affine point, or None."""
    if r >= P:
        return None
    alpha = (pow(r, 3, P) + 7) % P
    beta = pow(alpha, (P + 1) // 4, P)
    if (beta * beta) % P != alpha:
        return None
    y = beta if (beta & 1) == i else (P - beta) % P
    R = (r, y, 1)
    z = int.from_bytes(digest32, "big") % N
    # P = r^-1 (sR - zG) = (s/r) R + (-z/r) G
    rinv = pow(r, N - 2, N)
    u1 = (-z * rinv) % N
    u2 = (s * rinv) % N
    Q = jadd(jmul(u1, G), jmul(u2, R))
    return to_affine(Q)


# --------------------------------------------------------------------------
# RFC 6979 (HMAC-SHA256) deterministic signing, low-s
# --------------------------------------------------------------------------

def _hmac(key, data):
    return hmac.new(key, data, hashlib.sha256).digest()


def sign_digest(priv, digest32):
    """Return (r, s, v) with low-s and v in {27, 28}."""
    z = int.from_bytes(digest32, "big") % N
    xo = priv.to_bytes(32, "big")
    ho = z.to_bytes(32, "big")          # bits2octets(h1)
    V = b"\x01" * 32
    K = b"\x00" * 32
    K = _hmac(K, V + b"\x00" + xo + ho)
    V = _hmac(K, V)
    K = _hmac(K, V + b"\x01" + xo + ho)
    V = _hmac(K, V)
    while True:
        T = b""
        while len(T) < 32:
            V = _hmac(K, V)
            T += V
        k = int.from_bytes(T[:32], "big")
        if 1 <= k < N:
            pt = to_affine(jmul(k, G))
            if pt is not None:
                rx, ry = pt
                if rx < N and rx != 0:
                    r = rx
                    s = (pow(k, N - 2, N) * (z + r * priv)) % N
                    if s != 0:
                        recid = ry & 1
                        if s > HALF_N:
                            s = N - s
                            recid ^= 1
                        return (r, s, 27 + recid)
        K = _hmac(K, V + b"\x00")
        V = _hmac(K, V)
