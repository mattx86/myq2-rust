// md4.rs — RSA MD4 message-digest algorithm
// Converted from: myq2-original/qcommon/md4.c
// Now delegates to the `md4` crate (RustCrypto).

use md4::{Md4 as Md4Hasher, Digest};

/// MD4 context — wraps the `md4` crate's hasher.
pub struct Md4Context {
    hasher: Md4Hasher,
}

impl Md4Context {
    /// Create and initialize a new MD4 context.
    pub fn new() -> Self {
        Self {
            hasher: Md4Hasher::new(),
        }
    }

    /// Process a block of input data.
    pub fn update(&mut self, input: &[u8]) {
        self.hasher.update(input);
    }

    /// Finalize and return the 16-byte MD4 digest.
    pub fn finalize(self) -> [u8; 16] {
        let result = self.hasher.finalize();
        let mut digest = [0u8; 16];
        digest.copy_from_slice(&result);
        digest
    }
}

impl Default for Md4Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a block checksum by XORing all four MD4 digest words.
/// This is the Quake 2 `Com_BlockChecksum` function.
pub fn com_block_checksum(data: &[u8]) -> u32 {
    let mut ctx = Md4Context::new();
    ctx.update(data);
    let digest = ctx.finalize();

    let d0 = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
    let d1 = u32::from_le_bytes([digest[4], digest[5], digest[6], digest[7]]);
    let d2 = u32::from_le_bytes([digest[8], digest[9], digest[10], digest[11]]);
    let d3 = u32::from_le_bytes([digest[12], digest[13], digest[14], digest[15]]);

    d0 ^ d1 ^ d2 ^ d3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md4_empty() {
        let ctx = Md4Context::new();
        let digest = ctx.finalize();
        // MD4("") = 31d6cfe0d16ae931b73c59d7e0c089c0
        assert_eq!(
            digest,
            [
                0x31, 0xd6, 0xcf, 0xe0, 0xd1, 0x6a, 0xe9, 0x31, 0xb7, 0x3c, 0x59, 0xd7, 0xe0,
                0xc0, 0x89, 0xc0
            ]
        );
    }

    #[test]
    fn test_md4_abc() {
        let mut ctx = Md4Context::new();
        ctx.update(b"abc");
        let digest = ctx.finalize();
        // MD4("abc") = a448017aaf21d8525fc10ae87aa6729d
        assert_eq!(
            digest,
            [
                0xa4, 0x48, 0x01, 0x7a, 0xaf, 0x21, 0xd8, 0x52, 0x5f, 0xc1, 0x0a, 0xe8, 0x7a,
                0xa6, 0x72, 0x9d
            ]
        );
    }

    #[test]
    fn test_block_checksum() {
        let val = com_block_checksum(b"test");
        // Just verify it produces a non-zero value consistently
        assert_ne!(val, 0);
        assert_eq!(val, com_block_checksum(b"test"));
    }
}
