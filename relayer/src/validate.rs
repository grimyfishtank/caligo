//! Request validation for relay payloads.
//!
//! Validates proof format, address format, and fee parameters
//! before attempting to broadcast a withdrawal transaction.

use stellar_strkey::ed25519::PublicKey;

/// Validate that a hex string has the expected byte length.
pub fn validate_hex(hex_str: &str, expected_bytes: usize) -> Result<Vec<u8>, String> {
    let clean = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(clean).map_err(|e| format!("Invalid hex: {}", e))?;
    if bytes.len() != expected_bytes {
        return Err(format!(
            "Expected {} bytes, got {}",
            expected_bytes,
            bytes.len()
        ));
    }
    Ok(bytes)
}

/// Validate a Stellar address (G... public key format).
pub fn validate_stellar_address(address: &str) -> Result<(), String> {
    PublicKey::from_string(address)
        .map_err(|e| format!("Invalid Stellar address '{}': {}", address, e))?;
    Ok(())
}

/// Validate a relay request's fields (used by integration tests and future batch validation).
#[allow(dead_code)]
pub fn validate_relay_request(
    proof_hex: &str,
    root_hex: &str,
    nullifier_hash_hex: &str,
    recipient: &str,
    fee: &str,
    pool_contract_id: &str,
    relayer_fee_bps: u32,
    pool_denomination: u64,
) -> Result<(), String> {
    // Validate proof: 256 bytes
    validate_hex(proof_hex, 256)?;

    // Validate root: 32 bytes
    validate_hex(root_hex, 32)?;

    // Validate nullifier hash: 32 bytes
    validate_hex(nullifier_hash_hex, 32)?;

    // Validate recipient address
    validate_stellar_address(recipient)?;

    // Validate fee is a valid number
    let fee_stroops: u64 = fee
        .parse()
        .map_err(|_| format!("Invalid fee '{}': must be a positive integer in stroops", fee))?;

    // Validate fee matches expected relayer fee
    let expected_fee = (pool_denomination as u128 * relayer_fee_bps as u128) / 10000;
    if fee_stroops != expected_fee as u64 {
        return Err(format!(
            "Fee mismatch: expected {} stroops ({}bps of {}), got {}",
            expected_fee, relayer_fee_bps, pool_denomination, fee_stroops
        ));
    }

    // Validate pool contract ID is non-empty
    if pool_contract_id.is_empty() {
        return Err("Pool contract ID is required".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_hex_valid() {
        let result = validate_hex("aabbccdd", 4);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![0xaa, 0xbb, 0xcc, 0xdd]);
    }

    #[test]
    fn test_validate_hex_with_0x_prefix() {
        let result = validate_hex("0xaabbccdd", 4);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_hex_wrong_length() {
        let result = validate_hex("aabb", 4);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_hex_invalid() {
        let result = validate_hex("gggg", 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_stellar_address_valid() {
        // Standard Stellar test account address
        let result =
            validate_stellar_address("GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN7");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_stellar_address_invalid() {
        let result = validate_stellar_address("not-a-valid-address");
        assert!(result.is_err());
    }
}
