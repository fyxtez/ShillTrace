use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CandidateAddress {
    pub address: String,
    pub address_kind: AddressKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressKind { Evm, Solana }

static EVM: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b0x[a-f0-9]{40}\b").unwrap());
static SOLANA: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[1-9A-HJ-NP-Za-km-z]{32,44}\b").unwrap());

// Known chart URLs already contain the raw CA, so scanning the entire message
// supports Padre, DEX Screener, Pump.fun and ordinary free-form text together.
pub fn detect_addresses(text: &str) -> Vec<CandidateAddress> {
    let mut unique = HashSet::new();
    let mut result = Vec::new();

    for hit in EVM.find_iter(text) {
        let normalized = hit.as_str().to_lowercase();
        if unique.insert(normalized.clone()) {
            result.push(CandidateAddress { address: normalized, address_kind: AddressKind::Evm });
        }
    }

    for hit in SOLANA.find_iter(text) {
        let candidate = hit.as_str();
        if bs58::decode(candidate).into_vec().is_ok_and(|bytes| bytes.len() == 32)
            && unique.insert(candidate.to_owned())
        {
            result.push(CandidateAddress { address: candidate.to_owned(), address_kind: AddressKind::Solana });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_evm_and_solana_from_links() {
        let text = "https://trade.padre.gg/trade/base/0xB2000000000000000000004c27f6523082f41D01 https://trade.padre.gg/trade/solana/BQsfVh5rr3yDCSzmq8cdZwJGZq8nKzLAb7vKRWpzpump";
        assert_eq!(detect_addresses(text).len(), 2);
    }
}

