use crate::errors::Result;

pub trait SecurityProvider {
    fn seal_page_bytes(&self, page_bytes: &[u8]) -> Result<Vec<u8>>;
    fn open_page_bytes(&self, stored_bytes: &[u8]) -> Result<Vec<u8>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoSecurity;

impl SecurityProvider for NoSecurity {
    fn seal_page_bytes(&self, page_bytes: &[u8]) -> Result<Vec<u8>> {
        // Future pipeline hook: page bytes -> encryption/session policy -> stored bytes.
        // v0.1 deliberately stores plaintext bytes and does not pretend to encrypt.
        Ok(page_bytes.to_vec())
    }

    fn open_page_bytes(&self, stored_bytes: &[u8]) -> Result<Vec<u8>> {
        // Future pipeline hook: stored bytes -> authenticated decrypt -> page bytes.
        Ok(stored_bytes.to_vec())
    }
}
