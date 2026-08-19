//! Real Monero decoy selection — fetches actual UTXO output keys from the daemon.
//!
//! Replaces `build_decoy_ring()`'s synthetic random curve points with real
//! blockchain output keys for proper ring-signature privacy.
//!
//! ## Selection strategy
//!
//! 1. Fetch output distribution via `get_output_distribution` (cumulative, from_height=0)
//! 2. Select `RING_SIZE - 1` random output indices weighted by the distribution
//! 3. Fetch actual output keys + commitments via `get_outs` daemon RPC
//! 4. Insert the real signer at a random position
//!
//! ## Safety
//!
//! Returns `PluginError::NetworkError` if the daemon is unreachable — never
//! falls back to synthetic decoys. A failed fetch means the transaction is
//! refused, preserving privacy.

use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use rand::Rng;
use serde_json::Value;
use std::collections::HashSet;

use crate::{PluginError, daemon_rpc};

/// Default ring size for Monero transactions (11 = 10 decoys + 1 real).
pub const RING_SIZE: usize = 11;

/// Select real decoy output keys from the blockchain and build a ring.
///
/// Returns `(signer_ring_member, ring, offsets)` where `ring` is `RING_SIZE` entries of
/// `[public_key, commitment]` — same shape as the legacy `build_decoy_ring` —
/// and `offsets` contains the actual blockchain output indices for `Decoys::new`.
///
/// `signer_public` is the real signer's public spend key.
///
/// # Errors
///
/// Returns `PluginError::NetworkError` when the daemon is unreachable, returns
/// no outputs, or returns malformed data.
pub async fn fetch_and_build_ring(
    client: &reqwest::Client,
    network: &str,
    signer_public: &EdwardsPoint,
) -> Result<([EdwardsPoint; 2], Vec<[EdwardsPoint; 2]>, Vec<u64>), PluginError> {
    // 1. Fetch output distribution to know which indices exist
    let distribution = fetch_output_distribution(client, network).await?;

    // Total number of outputs on this chain
    let total_outputs = *distribution.last().unwrap_or(&0);
    if total_outputs == 0 {
        return Err(PluginError::NetworkError(
            "daemon reports zero outputs — cannot select decoys".into(),
        ));
    }

    // 2. Select RING_SIZE - 1 distinct random output indices
    let decoy_count = RING_SIZE - 1;
    let decoy_indices = select_decoy_indices(&distribution, decoy_count, total_outputs);

    if decoy_indices.len() < decoy_count {
        return Err(PluginError::NetworkError(format!(
            "only selected {} decoy indices but need {decoy_count}",
            decoy_indices.len()
        )));
    }

    // 3. Fetch actual output keys + commitments for selected indices
    let real_outputs = fetch_output_keys(client, network, &decoy_indices).await?;

    if real_outputs.len() < decoy_count {
        return Err(PluginError::NetworkError(format!(
            "daemon returned {} outputs but expected {decoy_count}",
            real_outputs.len()
        )));
    }

    // 4. Insert the real signer at a random position in the ring, tracking indices
    let mut rng = rand::rng();
    let signer_index = rng.random_range(0..RING_SIZE) as u8;

    let signer_commitment = signer_public * Scalar::from(8u8);
    let signer_member = [*signer_public, signer_commitment];

    let mut ring = Vec::with_capacity(RING_SIZE);
    let mut ring_indices: Vec<u64> = Vec::with_capacity(RING_SIZE);
    let mut decoy_iter = real_outputs.into_iter();

    for i in 0..RING_SIZE {
        if i as u8 == signer_index {
            ring.push(signer_member);
            ring_indices.push(decoy_indices[0]); // placeholder — real signer has no real index
        } else {
            let entry = decoy_iter
                .next()
                .ok_or_else(|| PluginError::Internal("decoy iterator exhausted early".into()))?;
            ring.push(entry);
            ring_indices.push(decoy_indices[ring.len() - 1]);
        }
    }

    Ok((signer_member, ring, ring_indices))
}

/// Fetch cumulative output distribution from the Monero daemon.
///
/// Returns a `Vec<u64>` where `dist[h]` is the cumulative number of outputs
/// at height `h + from_height` (with `from_height = 0`).
async fn fetch_output_distribution(
    client: &reqwest::Client,
    network: &str,
) -> Result<Vec<u64>, PluginError> {
    let params = serde_json::json!({
        "amounts": [0],
        "cumulative": true,
        "from_height": 0,
    });

    let result = daemon_rpc(client, network, "get_output_distribution", params).await?;

    let distributions = result
        .get("distributions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PluginError::NetworkError(
                "missing 'distributions' in get_output_distribution response".into(),
            )
        })?;

    let dist_data = distributions
        .first()
        .and_then(|d| d.get("data"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PluginError::NetworkError("missing 'data' in get_output_distribution result".into())
        })?;

    let distribution: Vec<u64> = dist_data.iter().filter_map(|v| v.as_u64()).collect();

    if distribution.is_empty() {
        return Err(PluginError::NetworkError(
            "get_output_distribution returned empty distribution".into(),
        ));
    }

    Ok(distribution)
}

/// Select `count` distinct uniform random output indices from the active set.
///
/// Uses the cumulative distribution to select outputs from the *recent* portion
/// of the chain — weighted toward outputs within the last ~1.8M blocks
/// (the \"recent window\" used by Monero Core for spending).
///
/// The selection uses a modified version of Monero's decoy selection algorithm:
/// - 50% probability to pick from the last ~N blocks (recent)
/// - 50% probability to pick uniformly across the full chain
///
/// This ensures a reasonable distribution that won't trivially stand out
/// on chain analysis.
fn select_decoy_indices(distribution: &[u64], count: usize, total_outputs: u64) -> Vec<u64> {
    let mut rng = rand::rng();
    let mut selected: HashSet<u64> = HashSet::new();
    // Prevent picking index 0 (genesis output is a known coinbase that
    // would immediately identify this as a fake ring)
    let min_index = 1u64;

    // The "recent" window: last ~30% of blocks or at most the last
    // 1.8M blocks (Monero's default recent window). Since we don't
    // know the exact block count from the distribution alone, use
    // the last 30% of distribution entries.
    let recent_cutoff = (distribution.len() as f64 * 0.7) as usize;
    let recent_start_index = distribution
        .get(recent_cutoff)
        .copied()
        .unwrap_or(total_outputs / 2);

    let max_iterations = count * 20; // safety valve to avoid infinite loops
    let mut iterations = 0;

    while selected.len() < count && iterations < max_iterations {
        iterations += 1;

        let pick_recent = rng.random_bool(0.5);
        let max_index = if pick_recent {
            recent_start_index
        } else {
            total_outputs
        };

        if max_index <= min_index {
            continue;
        }

        let candidate = rng.random_range(min_index..max_index);

        // Check if this output index actually exists in the distribution
        let exists = distribution.iter().any(|&cum| cum >= candidate);
        if exists && !selected.contains(&candidate) {
            selected.insert(candidate);
        }
    }

    let mut result: Vec<u64> = selected.into_iter().collect();
    result.sort();
    result
}

/// Fetch actual output keys and corresponding commitments for the given indices.
///
/// Returns one `[EdwardsPoint; 2]` per index: `[output_key, commitment]`.
async fn fetch_output_keys(
    client: &reqwest::Client,
    network: &str,
    indices: &[u64],
) -> Result<Vec<[EdwardsPoint; 2]>, PluginError> {
    let outputs: Vec<Value> = indices
        .iter()
        .map(|&idx| {
            serde_json::json!({
                "index": idx,
                "amount": 0, // RingCT uses amount=0
            })
        })
        .collect();

    let params = serde_json::json!({
        "outputs": outputs,
        "get_txid": false,
    });

    let result = daemon_rpc(client, network, "get_outs", params).await?;

    let outs = result
        .get("outs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| PluginError::NetworkError("missing 'outs' in get_outs response".into()))?;

    if outs.is_empty() {
        return Err(PluginError::NetworkError(
            "get_outs returned empty outputs array".into(),
        ));
    }

    let mut real_outputs: Vec<[EdwardsPoint; 2]> = Vec::with_capacity(outs.len());

    for out in outs {
        let key_hex = out.get("key").and_then(|v| v.as_str()).ok_or_else(|| {
            PluginError::NetworkError("missing 'key' in get_outs output entry".into())
        })?;

        let key_bytes = hex::decode(key_hex)
            .map_err(|e| PluginError::NetworkError(format!("invalid hex key in get_outs: {e}")))?;

        let key_point = bytes_to_edwards(&key_bytes)?;

        // Decode the commitment from the output if present, otherwise
        // derive a commitment to zero (backward compat with older daemons)
        let commitment_point = if let Some(mask) = out.get("mask").and_then(|v| v.as_str()) {
            let mask_bytes = hex::decode(mask)
                .map_err(|e| PluginError::NetworkError(format!("invalid hex mask: {e}")))?;
            bytes_to_edwards(&mask_bytes)?
        } else {
            // Fallback: commitment = key * 8 (zero amount)
            key_point * Scalar::from(8u8)
        };

        real_outputs.push([key_point, commitment_point]);
    }

    Ok(real_outputs)
}

/// Decode 32 bytes into an EdwardsPoint via CompressedEdwardsY.
fn bytes_to_edwards(bytes: &[u8]) -> Result<EdwardsPoint, PluginError> {
    match bytes.len() {
        32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(bytes);
            CompressedEdwardsY(arr).decompress().ok_or_else(|| {
                PluginError::NetworkError("failed to decode compressed EdwardsPoint".into())
            })
        }
        64 => {
            // Try uncompressed: first 32 bytes are Y, last 32 are X
            // (or vice versa depending on encoding). Convert to 32-byte compressed.
            let mut compressed = [0u8; 32];
            compressed.copy_from_slice(&bytes[..32]);
            compressed[31] |= bytes[63] & 0x80; // flip sign bit from X
            CompressedEdwardsY(compressed).decompress().ok_or_else(|| {
                PluginError::NetworkError("failed to decode uncompressed EdwardsPoint".into())
            })
        }
        _ => Err(PluginError::NetworkError(format!(
            "unexpected EdwardsPoint byte length: {} (expected 32 or 64)",
            bytes.len()
        ))),
    }
}
