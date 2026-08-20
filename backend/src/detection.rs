use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CandidateAddress {
    pub address: String,
    pub address_kind: AddressKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressKind {
    Evm,
    Solana,
    Tron,
}

static EVM: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b0x[a-f0-9]{40}\b").unwrap());
static SOLANA: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[1-9A-HJ-NP-Za-km-z]{32,44}\b").unwrap());
static TRON: Lazy<Regex> = Lazy::new(|| Regex::new(r"\bT[1-9A-HJ-NP-Za-km-z]{33}\b").unwrap());

// Known chart URLs already contain the raw CA, so scanning the entire message
// supports Padre, DEX Screener, Pump.fun and ordinary free-form text together.
pub fn detect_addresses(text: &str) -> Vec<CandidateAddress> {
    let mut unique = HashSet::new();
    let mut result = Vec::new();

    for hit in EVM.find_iter(text) {
        let normalized = hit.as_str().to_lowercase();
        if unique.insert(normalized.clone()) {
            result.push(CandidateAddress {
                address: normalized,
                address_kind: AddressKind::Evm,
            });
        }
    }

    // TRON addresses share Base58 characters with Solana, so validate their
    // version byte and four-byte checksum first and consume them before the
    // broader Solana scan. This prevents a valid TRX/TRC-20 contract from
    // being mislabeled as Solana and rejects lookalike text.
    for hit in TRON.find_iter(text) {
        let candidate = hit.as_str();
        if is_valid_tron_address(candidate) && unique.insert(candidate.to_owned()) {
            result.push(CandidateAddress {
                address: candidate.to_owned(),
                address_kind: AddressKind::Tron,
            });
        }
    }

    for hit in SOLANA.find_iter(text) {
        let candidate = hit.as_str();
        if bs58::decode(candidate)
            .into_vec()
            .is_ok_and(|bytes| bytes.len() == 32)
            && unique.insert(candidate.to_owned())
        {
            result.push(CandidateAddress {
                address: candidate.to_owned(),
                address_kind: AddressKind::Solana,
            });
        }
    }
    result
}


// EVM addresses are chain-ambiguous by themselves. We only attempt wallet
// verification when the message provides an explorer hint, while Solana has a
// unique address format and can be probed directly in the background.
pub fn wallet_chain_hint(text: &str, kind: AddressKind) -> Option<&'static str> {
    match kind {
        AddressKind::Solana => Some("solana"),
        AddressKind::Tron => None,
        AddressKind::Evm => {
            let lower = text.to_ascii_lowercase();
            if lower.contains("optimistic.etherscan.io") {
                Some("optimism")
            } else if lower.contains("etherscan.io") {
                Some("ethereum")
            } else if lower.contains("basescan.org") {
                Some("base")
            } else if lower.contains("bscscan.com") {
                Some("bsc")
            } else if lower.contains("arbiscan.io") {
                Some("arbitrum")
            } else if lower.contains("polygonscan.com") {
                Some("polygon")
            } else {
                None
            }
        }
    }
}

fn is_valid_tron_address(candidate: &str) -> bool {
    let Ok(decoded) = bs58::decode(candidate).into_vec() else {
        return false;
    };
    if decoded.len() != 25 || decoded[0] != 0x41 {
        return false;
    }
    let first_hash = Sha256::digest(&decoded[..21]);
    let second_hash = Sha256::digest(first_hash);
    decoded[21..] == second_hash[..4]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_evm_and_solana_from_links() {
        let text = "https://trade.padre.gg/trade/base/0xB2000000000000000000004c27f6523082f41D01 https://trade.padre.gg/trade/solana/BQsfVh5rr3yDCSzmq8cdZwJGZq8nKzLAb7vKRWpzpump";
        assert_eq!(detect_addresses(text).len(), 2);
    }

    #[test]
    fn detects_valid_tron_address_as_tron_only() {
        // A real Base58Check TRON address verifies both chain recognition and
        // deduplication against the overlapping Solana Base58 expression.
        let detected = detect_addresses("TRX token: TJRabPrwbZy45sbavfcjinPJC18kjpRTv8");
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].address_kind, AddressKind::Tron);
    }

    #[test]
    fn infers_wallet_chain_only_from_safe_context() {
        // A raw 0x address can exist on many EVM chains, so an explorer URL is
        // required before background RPC classification chooses a network.
        assert_eq!(wallet_chain_hint("https://etherscan.io/address/0x0000000000000000000000000000000000000001", AddressKind::Evm), Some("ethereum"));
        assert_eq!(wallet_chain_hint("0x0000000000000000000000000000000000000001", AddressKind::Evm), None);
    }

    #[test]
    fn rejects_tron_lookalike_with_bad_checksum() {
        // Changing the final Base58 character must not create a token from a
        // typo or malicious lookalike posted in a monitored channel.
        assert!(detect_addresses("TJRabPrwbZy45sbavfcjinPJC18kjpRTv9").is_empty());
    }
}
