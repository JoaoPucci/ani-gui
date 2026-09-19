//! The cipher hianime's megaplay sources answer is written under:
//! the constants the site's own player script opens it with, and the
//! plaintext they open a blob to. What the plaintext says is
//! [`super::megaplay_sources`]'s to read.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut as _, KeyIvInit as _};
use base64::Engine as _;

/// The key the site's player opens its sources answer with, as its
/// script carries it: sixteen ASCII bytes, zero-padded to the
/// cipher's width. The site's own constant, used there for its
/// segment tokens as well, so a rotation shows up in both at once.
const SOURCES_KEY: &[u8; 16] = b"i?LMTAx0Q6,:}50U";

/// The initialisation vector beside it, likewise the site's own
/// constant: fixed for every answer, which is why one captured
/// ciphertext is enough to hold the pair to account.
const SOURCES_IV: &[u8; 16] = b"W0;27ToaUpl_P%\'c";

/// The cipher the answer is under.
type SourcesCipher = cbc::Decryptor<aes::Aes256>;

/// How the site writes the ciphertext out: base64 over the URL
/// alphabet, padding optional — the answers seen carry none, and one
/// that carried it would be the same bytes.
const SOURCES_B64: base64::engine::GeneralPurpose = base64::engine::GeneralPurpose::new(
    &base64::alphabet::URL_SAFE,
    base64::engine::general_purpose::NO_PAD
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
);

/// The plaintext a blob opens to. `Err` carries what failed, for the
/// caller to report as it reports the rest of a sources answer it
/// cannot read: each is the site having changed something different —
/// how it writes the blob out, how long a blob it writes, or the
/// constants it writes it under.
///
/// # Errors
/// The reason, when the blob is not base64 over the URL alphabet, not
/// a whole number of cipher blocks, or not one the constants open.
pub fn opened(enc: &str) -> std::result::Result<Vec<u8>, &'static str> {
    let bytes = SOURCES_B64
        .decode(enc)
        .map_err(|_| "the encrypted sources are not base64")?;
    // Checked before the cipher runs so that a blob of the wrong
    // length and one the constants do not open are told apart: the
    // unpadding refuses both with the one error, and they send
    // whoever reads the failure to different places.
    if bytes.is_empty() || bytes.len() % SOURCES_IV.len() != 0 {
        return Err("the encrypted sources are not whole cipher blocks");
    }
    let mut key = [0u8; 32];
    key[..SOURCES_KEY.len()].copy_from_slice(SOURCES_KEY);
    SourcesCipher::new(&key.into(), SOURCES_IV.into())
        .decrypt_padded_vec_mut::<Pkcs7>(&bytes)
        .map_err(|_| "the encrypted sources do not open under the site's key")
}
