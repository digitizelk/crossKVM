use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionError,
    #[error("Decryption failed / auth tag mismatch")]
    DecryptionError,
    #[error("Invalid key or nonce")]
    InvalidData,
}

pub struct PacketCipher {
    cipher: Aes256Gcm,
}

impl PacketCipher {
    /// Derive a 256-bit AES key from a passphrase using SHA-256
    pub fn from_passphrase(passphrase: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"cross-kvm-salt-v1");
        hasher.update(passphrase.as_bytes());
        let hash = hasher.finalize();

        let key = Key::<Aes256Gcm>::from_slice(&hash);
        let cipher = Aes256Gcm::new(key);
        Self { cipher }
    }

    /// Encrypt plaintext. Prepends 12-byte random nonce to ciphertext.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::EncryptionError)?;

        let mut output = Vec::with_capacity(12 + ciphertext.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    /// Decrypt payload where the first 12 bytes are the nonce.
    pub fn decrypt(&self, payload: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if payload.len() < 12 {
            return Err(CryptoError::InvalidData);
        }

        let (nonce_slice, ciphertext) = payload.split_at(12);
        let nonce = Nonce::from_slice(nonce_slice);

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| CryptoError::DecryptionError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_roundtrip() {
        let cipher = PacketCipher::from_passphrase("my-secure-password");
        let original = b"Hello, secure cross-kvm stream!";

        let encrypted = cipher.encrypt(original).expect("encryption should succeed");
        assert_ne!(encrypted.as_slice(), original.as_slice());

        let decrypted = cipher.decrypt(&encrypted).expect("decryption should succeed");
        assert_eq!(decrypted.as_slice(), original.as_slice());
    }

    #[test]
    fn test_wrong_passphrase_fails() {
        let cipher1 = PacketCipher::from_passphrase("password-1");
        let cipher2 = PacketCipher::from_passphrase("password-2");

        let encrypted = cipher1.encrypt(b"secret").unwrap();
        let result = cipher2.decrypt(&encrypted);
        assert!(result.is_err());
    }
}
