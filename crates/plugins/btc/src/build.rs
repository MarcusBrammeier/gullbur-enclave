//! Bitcoin transaction construction: coin selection, fee, change, unsigned PSBT
//! assembly, and PSBT finalisation.
//!
//! This module is the layer the product never had. Before it existed, signing
//! stopped at an unfinalised PSBT and `broadcast_transaction` posted those PSBT
//! bytes to Esplora as if they were a raw transaction.

use bitcoin::absolute::LockTime;
use bitcoin::psbt::Psbt;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, OutPoint, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
};

/// Approximate vsize of a P2WPKH input: 41 vB base + 107 WU witness / 4 ≈ 68 vB.
pub const P2WPKH_INPUT_VSIZE: u64 = 68;
/// P2WPKH output: 8 (value) + 1 (len) + 22 (script) = 31 vB.
pub const P2WPKH_OUTPUT_VSIZE: u64 = 31;
/// Version + locktime + varint counts.
pub const TX_OVERHEAD_VSIZE: u64 = 11;
/// Conservative dust limit for a P2WPKH output.
pub const DUST_LIMIT_SATS: u64 = 546;

/// A spendable output belonging to the sending account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utxo {
    pub txid: Txid,
    pub vout: u32,
    pub value: u64,
}

/// Result of coin selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub inputs: Vec<Utxo>,
    pub fee_sats: u64,
    pub change_sats: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BuildError {
    NoUtxos,
    InsufficientFunds { available: u64, required: u64 },
    Address(String),
    Finalize(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::NoUtxos => write!(f, "account has no spendable outputs"),
            BuildError::InsufficientFunds {
                available,
                required,
            } => write!(
                f,
                "insufficient funds: {available} sats available, {required} sats required (incl. fee)"
            ),
            BuildError::Address(e) => write!(f, "invalid address: {e}"),
            BuildError::Finalize(e) => write!(f, "PSBT finalisation failed: {e}"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Estimated virtual size for a transaction with `n_in` P2WPKH inputs.
pub fn estimated_vsize(n_in: u64, n_out: u64) -> u64 {
    n_in * P2WPKH_INPUT_VSIZE + n_out * P2WPKH_OUTPUT_VSIZE + TX_OVERHEAD_VSIZE
}

/// Fee in sats for the given shape at `sat_per_vb`, rounded up.
pub fn fee_for(n_in: u64, n_out: u64, sat_per_vb: f64) -> u64 {
    let rate = if sat_per_vb.is_finite() && sat_per_vb > 0.0 {
        sat_per_vb
    } else {
        1.0
    };
    (estimated_vsize(n_in, n_out) as f64 * rate).ceil() as u64
}

/// Select outputs largest-first until the amount plus fee is covered.
///
/// Change below the dust limit is absorbed into the fee rather than created as
/// an unspendable output.
pub fn select_coins(
    utxos: &[Utxo],
    amount: u64,
    sat_per_vb: f64,
) -> Result<Selection, BuildError> {
    if utxos.is_empty() {
        return Err(BuildError::NoUtxos);
    }
    if amount == 0 {
        return Err(BuildError::InsufficientFunds {
            available: 0,
            required: 0,
        });
    }

    let mut sorted: Vec<Utxo> = utxos.iter().copied().filter(|u| u.value > 0).collect();
    if sorted.is_empty() {
        return Err(BuildError::NoUtxos);
    }
    sorted.sort_by(|a, b| b.value.cmp(&a.value));

    let mut total: u64 = 0;
    for (i, utxo) in sorted.iter().enumerate() {
        total = total.saturating_add(utxo.value);
        let n_in = (i as u64) + 1;

        let fee_with_change = fee_for(n_in, 2, sat_per_vb);
        let needed = amount.saturating_add(fee_with_change);
        if total >= needed {
            let change = total - needed;
            if change >= DUST_LIMIT_SATS {
                return Ok(Selection {
                    inputs: sorted[..=i].to_vec(),
                    fee_sats: fee_with_change,
                    change_sats: change,
                });
            }
            // Change too small to be worth an output: drop it into the fee.
            let fee_no_change = fee_for(n_in, 1, sat_per_vb);
            if total >= amount.saturating_add(fee_no_change) {
                return Ok(Selection {
                    inputs: sorted[..=i].to_vec(),
                    fee_sats: total - amount,
                    change_sats: 0,
                });
            }
        }
    }

    Err(BuildError::InsufficientFunds {
        available: total,
        required: amount.saturating_add(fee_for(sorted.len() as u64, 2, sat_per_vb)),
    })
}

/// Assemble an unsigned PSBT spending `selection` to `to`, with change back to
/// `from` when the selection produces change.
pub fn build_unsigned_psbt(
    from: &Address,
    to: &Address,
    selection: &Selection,
    amount: u64,
) -> Result<Vec<u8>, BuildError> {
    if selection.inputs.is_empty() {
        return Err(BuildError::NoUtxos);
    }

    let input: Vec<TxIn> = selection
        .inputs
        .iter()
        .map(|u| TxIn {
            previous_output: OutPoint {
                txid: u.txid,
                vout: u.vout,
            },
            script_sig: Default::default(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        })
        .collect();

    let mut output = vec![TxOut {
        value: Amount::from_sat(amount),
        script_pubkey: to.script_pubkey(),
    }];
    if selection.change_sats > 0 {
        output.push(TxOut {
            value: Amount::from_sat(selection.change_sats),
            script_pubkey: from.script_pubkey(),
        });
    }

    let unsigned = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input,
        output,
    };

    let mut psbt = Psbt::from_unsigned_tx(unsigned)
        .map_err(|e| BuildError::Finalize(format!("PSBT construction failed: {e}")))?;

    // Every input is ours, so its witness UTXO is known up front. Without this
    // the signer cannot compute a BIP-143 sighash.
    for (i, utxo) in selection.inputs.iter().enumerate() {
        psbt.inputs[i].witness_utxo = Some(TxOut {
            value: Amount::from_sat(utxo.value),
            script_pubkey: from.script_pubkey(),
        });
    }

    Ok(psbt.serialize())
}

/// Finalise a signed PSBT into broadcastable raw transaction bytes.
///
/// `extract_tx()` is **not** a finalisation gate: rust-bitcoin will happily
/// extract a transaction whose inputs carry no witness, producing bytes that
/// every node rejects. This function performs the actual finalisation — moving
/// each input's partial signature into a witness — and refuses to return bytes
/// that are not spendable.
pub fn finalize_psbt(signed_psbt: &[u8]) -> Result<Vec<u8>, BuildError> {
    let mut psbt = Psbt::deserialize(signed_psbt)
        .map_err(|e| BuildError::Finalize(format!("PSBT parse error: {e}")))?;

    for (i, input) in psbt.inputs.iter_mut().enumerate() {
        let already_final = input
            .final_script_witness
            .as_ref()
            .is_some_and(|w| !w.is_empty())
            || input
                .final_script_sig
                .as_ref()
                .is_some_and(|s| !s.is_empty());
        if already_final {
            continue;
        }

        // P2WPKH: exactly one partial signature, witness = [sig, pubkey].
        if input.partial_sigs.len() != 1 {
            return Err(BuildError::Finalize(format!(
                "input {i} has {} partial signatures; expected exactly 1 (was the PSBT signed?)",
                input.partial_sigs.len()
            )));
        }
        let (pubkey, sig) = input
            .partial_sigs
            .iter()
            .next()
            .expect("length checked above");
        let mut witness = Witness::new();
        witness.push(sig.serialize());
        witness.push(pubkey.to_bytes());
        input.final_script_witness = Some(witness);
        input.partial_sigs.clear();
    }

    let tx = psbt
        .extract_tx()
        .map_err(|e| BuildError::Finalize(format!("{e}")))?;

    // Belt and braces: every input must now carry witness data or a scriptSig.
    for (i, txin) in tx.input.iter().enumerate() {
        if txin.witness.is_empty() && txin.script_sig.is_empty() {
            return Err(BuildError::Finalize(format!(
                "input {i} extracted with no witness — refusing to broadcast an invalid transaction"
            )));
        }
    }

    Ok(bitcoin::consensus::serialize(&tx))
}

/// Total value of the selected inputs.
pub fn input_total(selection: &Selection) -> u64 {
    selection.inputs.iter().map(|u| u.value).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash as _;
    use std::str::FromStr;

    fn utxo(n: u8, value: u64) -> Utxo {
        Utxo {
            txid: Txid::from_byte_array([n; 32]),
            vout: 0,
            value,
        }
    }

    fn addr(s: &str) -> Address {
        Address::from_str(s)
            .expect("test fixture address parses")
            .assume_checked()
    }

    const FROM: &str = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
    const TO: &str = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";

    #[test]
    fn fee_scales_with_shape() {
        assert_eq!(estimated_vsize(1, 2), 11 + 68 + 62);
        assert!(fee_for(2, 2, 2.0) > fee_for(1, 2, 2.0));
        assert_eq!(fee_for(0, 0, 0.0), TX_OVERHEAD_VSIZE); // invalid rate floors to 1 sat/vB
    }

    #[test]
    fn single_utxo_produces_change() {
        let sel = select_coins(&[utxo(1, 100_000)], 50_000, 1.0).expect("selectable");
        assert_eq!(sel.inputs.len(), 1);
        assert!(sel.change_sats >= DUST_LIMIT_SATS);
        assert_eq!(
            input_total(&sel),
            sel.fee_sats + 50_000 + sel.change_sats,
            "value must balance exactly"
        );
    }

    #[test]
    fn dust_change_is_absorbed_into_fee() {
        // Amount picked so that change would land under the dust limit.
        let one_out_fee = fee_for(1, 1, 1.0);
        let amount = 100_000 - one_out_fee - 100; // 100 sats of change → dust
        let sel = select_coins(&[utxo(1, 100_000)], amount, 1.0).expect("selectable");
        assert_eq!(sel.change_sats, 0, "dust change must not become an output");
        assert_eq!(sel.fee_sats, 100_000 - amount);
        assert!(sel.fee_sats > one_out_fee);
    }

    #[test]
    fn selects_multiple_inputs_when_needed() {
        let utxos = [utxo(1, 30_000), utxo(2, 30_000), utxo(3, 30_000)];
        let sel = select_coins(&utxos, 70_000, 1.0).expect("selectable");
        assert!(sel.inputs.len() >= 3);
        assert_eq!(input_total(&sel), sel.fee_sats + 70_000 + sel.change_sats);
    }

    #[test]
    fn insufficient_funds_is_an_error() {
        let err = select_coins(&[utxo(1, 1_000)], 50_000, 5.0).expect_err("must fail");
        match err {
            BuildError::InsufficientFunds {
                available,
                required,
            } => {
                assert_eq!(available, 1_000);
                assert!(required > 50_000);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn empty_wallet_is_an_error() {
        assert_eq!(select_coins(&[], 1_000, 1.0), Err(BuildError::NoUtxos));
    }

    #[test]
    fn builds_psbt_with_witness_utxos() {
        let sel = select_coins(&[utxo(1, 100_000)], 50_000, 1.0).expect("selectable");
        let psbt_bytes =
            build_unsigned_psbt(&addr(FROM), &addr(TO), &sel, 50_000).expect("builds");
        let psbt = Psbt::deserialize(&psbt_bytes).expect("parses");

        assert_eq!(psbt.inputs.len(), 1);
        assert_eq!(psbt.unsigned_tx.output.len(), 2);
        assert_eq!(psbt.unsigned_tx.output[0].value.to_sat(), 50_000);
        assert!(
            psbt.inputs[0].witness_utxo.is_some(),
            "signer needs a witness UTXO on every input"
        );
        assert!(psbt.inputs[0].partial_sigs.is_empty(), "must be unsigned");
    }

    #[test]
    fn unsigned_psbt_cannot_be_finalized() {
        let sel = select_coins(&[utxo(1, 100_000)], 50_000, 1.0).expect("selectable");
        let psbt_bytes = build_unsigned_psbt(&addr(FROM), &addr(TO), &sel, 50_000).expect("builds");
        assert!(
            finalize_psbt(&psbt_bytes).is_err(),
            "finalising without signatures must fail, never silently succeed"
        );
    }

    #[test]
    fn finalize_rejects_garbage() {
        assert!(finalize_psbt(b"not a psbt").is_err());
    }

    /// The money test: build → sign → finalise → parse the raw transaction back
    /// and assert it actually carries a witness. No hand-built transactions.
    #[test]
    fn full_path_produces_a_spendable_transaction() {
        use bitcoin::ecdsa::Signature as BtcSignature;
        use bitcoin::sighash::{EcdsaSighashType, SighashCache};
        use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};
        use bitcoin::{CompressedPublicKey, Network};

        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(&[1u8; 32]).expect("test key");
        let pk = bitcoin::PublicKey::new(bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk));
        let owner = Address::p2wpkh(&CompressedPublicKey(pk.inner), Network::Testnet4);

        // One 100k-sat output owned by us; spend 50k back to ourselves.
        let funding = Utxo {
            txid: Txid::from_byte_array([7u8; 32]),
            vout: 0,
            value: 100_000,
        };
        let selection = select_coins(&[funding], 50_000, 2.0).expect("selectable");
        let psbt_bytes = build_unsigned_psbt(&owner, &owner, &selection, 50_000).expect("builds");

        // Sign exactly as the plugin does: BIP-143 sighash per input.
        let mut psbt = Psbt::deserialize(&psbt_bytes).expect("parses");
        let witness_utxo = psbt.inputs[0].witness_utxo.clone().expect("witness utxo");
        let sighash = {
            let mut cache = SighashCache::new(&psbt.unsigned_tx);
            cache
                .p2wpkh_signature_hash(
                    0,
                    &witness_utxo.script_pubkey,
                    witness_utxo.value,
                    EcdsaSighashType::All,
                )
                .expect("sighash")
        };
        let msg = Message::from_digest(*sighash.as_ref());
        psbt.inputs[0].partial_sigs.insert(
            pk,
            BtcSignature {
                signature: secp.sign_ecdsa(&msg, &sk),
                sighash_type: EcdsaSighashType::All,
            },
        );

        let raw = finalize_psbt(&psbt.serialize()).expect("finalises a signed PSBT");
        let tx: Transaction =
            bitcoin::consensus::deserialize(&raw).expect("output must be a valid transaction");

        assert_eq!(tx.input.len(), 1, "one selected input");
        assert!(
            !tx.input[0].witness.is_empty(),
            "finalised input must carry a witness"
        );
        assert_eq!(tx.input[0].witness.len(), 2, "P2WPKH witness = [sig, pubkey]");

        let spent: u64 = selection.inputs.iter().map(|u| u.value).sum();
        let out: u64 = tx.output.iter().map(|o| o.value.to_sat()).sum();
        assert_eq!(
            spent - out,
            selection.fee_sats,
            "fee paid must equal the selected fee"
        );
        assert_eq!(tx.output[0].value.to_sat(), 50_000, "payment amount intact");
    }
}
