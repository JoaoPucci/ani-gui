# Make minisign-format test vectors with throwaway keys (Ed25519 over
# BLAKE2b-512, the prehashed "ED" algorithm). Test fixtures only.
import base64, hashlib, os, sys
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives import serialization

def keypair(seed):
    sk = Ed25519PrivateKey.from_private_bytes(hashlib.sha256(seed).digest())
    pk = sk.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    key_id = hashlib.sha256(b'id' + seed).digest()[:8]
    return sk, pk, key_id

def pubkey_b64(pk, key_id):
    return base64.b64encode(b'Ed' + key_id + pk).decode()

def sign(sk, key_id, data, name='flags.json'):
    trusted = b'timestamp:1793700000\tfile:' + name.encode()
    sig = sk.sign(hashlib.blake2b(data, digest_size=64).digest())
    global_sig = sk.sign(sig + trusted)
    return ('untrusted comment: signature from minisign secret key\n'
            + base64.b64encode(b'ED' + key_id + sig).decode() + '\n'
            + 'trusted comment: ' + trusted.decode() + '\n'
            + base64.b64encode(global_sig).decode() + '\n')

if __name__ == '__main__':
    out = sys.argv[1]
    data = open(sys.argv[2], 'rb').read()
    # The signatures are named after the file signed: flags.json unless
    # a third argument names another.
    name = sys.argv[3] if len(sys.argv) > 3 else 'flags.json'
    sk, pk, kid = keypair(b'ani-gui test key one')
    sk2, pk2, kid2 = keypair(b'ani-gui test key two')
    sk3, pk3, kid3 = keypair(b'ani-gui test-run key')
    open(os.path.join(out, 'test-key-one.pub'), 'w').write(pubkey_b64(pk, kid) + '\n')
    open(os.path.join(out, 'test-key-two.pub'), 'w').write(pubkey_b64(pk2, kid2) + '\n')
    open(os.path.join(out, 'test-run-key.pub'), 'w').write(pubkey_b64(pk3, kid3) + '\n')
    open(os.path.join(out, name + '.minisig'), 'w').write(sign(sk, kid, data, name))
    open(os.path.join(out, name + '.by-key-two.minisig'), 'w').write(sign(sk2, kid2, data, name))
    open(os.path.join(out, name + '.by-test-run-key.minisig'), 'w').write(sign(sk3, kid3, data, name))
