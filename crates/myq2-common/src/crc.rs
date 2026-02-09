// crc.rs — 16-bit CCITT CRC (polynomial 0x1021)
// Converted from: myq2-original/qcommon/crc.c
// Now delegates to the `crc` crate (CRC-16/IBM-3740 == CRC-16/CCITT-FALSE).

use crc::{Crc, CRC_16_IBM_3740};

const CRC_CALC: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_3740);

/// Initialize a CRC value.
#[inline]
pub fn crc_init() -> u16 {
    0xffff
}

/// Process a single byte into the CRC.
#[inline]
pub fn crc_process_byte(crc: u16, data: u8) -> u16 {
    // Maintain the original byte-at-a-time API using the crc crate's digest.
    // The crc crate's internal table matches the original CRC_TABLE exactly.
    let mut digest = CRC_CALC.digest_with_initial(crc);
    digest.update(&[data]);
    digest.finalize()
}

/// Finalize and return the CRC value.
#[inline]
pub fn crc_value(crc: u16) -> u16 {
    crc // XOR value is 0x0000
}

/// Compute CRC for an entire block of data.
pub fn crc_block(data: &[u8]) -> u16 {
    CRC_CALC.checksum(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_empty() {
        let crc = crc_block(&[]);
        assert_eq!(crc, 0xffff); // CRC_INIT_VALUE
    }

    #[test]
    fn test_crc_consistency() {
        let data = b"Hello, World!";
        let crc1 = crc_block(data);
        let crc2 = crc_block(data);
        assert_eq!(crc1, crc2);
        assert_ne!(crc1, 0);
    }

    #[test]
    fn test_crc_byte_by_byte() {
        let data = b"test data";
        let block_crc = crc_block(data);

        let mut crc = crc_init();
        for &b in data {
            crc = crc_process_byte(crc, b);
        }
        assert_eq!(crc_value(crc), block_crc);
    }

    #[test]
    fn test_crc_check_value() {
        // The standard check value for CRC-16/CCITT-FALSE is 0x29B1
        // when computed over "123456789".
        let crc = crc_block(b"123456789");
        assert_eq!(crc, 0x29B1);
    }
}
