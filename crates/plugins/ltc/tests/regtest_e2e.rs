//! LTC E2E sign test against a LOCAL litecoind REGTEST node.
//!
//! Funds our derived P2PKH address (mkenEChN3CvkNr2hKxUkT72phJ5cZwGgXT) by mining.
//! Builds a PSBT spending one funded UTXO, signs it with the plugin's legacy
//! P2PKH path, prints the fully-finalized signed tx hex for the caller to
//! broadcast via `litecoin-cli sendrawtransaction` on the regtest node.
//!
//! Env:
//!   LTC_TESTNET_SEED  - hex seed (same BIP-39). Derived address must be the funded one.
//!   LTC_PREV_TXID     - funding txid
//!   LTC_PREV_VOUT     - output index of the funded output
//!   LTC_PREV_VALUE    - value in satoshis of that output

use bitcoin::absolute::LockTime;
use bitcoin::psbt::Psbt;
use bitcoin::transaction::Version;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness};
use plugin_ltc::LtcPlugin;
use wallet_plugin::WalletPlugin;

/// Recompute a P2PKH script_pubkey from a legacy m/n address (version 0x6f).
fn p2pkh_script_from_address(addr: &str) -> Result<ScriptBuf, String> {
    let decoded = base58ck::decode_check(addr).map_err(|e| format!("base58 decode: {e}"))?;
    if decoded.len() != 21 || decoded[0] != 0x6f {
        return Err(format!(
            "not a valid P2PKH address (len {} ver {}): {addr}",
            decoded.len(),
            decoded.get(0).map(|b| *b).unwrap_or(0)
        ));
    }
    let mut script = vec![0x76, 0xa9, 0x14]; // OP_DUP OP_HASH160 PUSH20
    script.extend_from_slice(&decoded[1..21]);
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG
    Ok(ScriptBuf::from_bytes(script))
}

fn push_cs(v: &mut Vec<u8>, d: &[u8]) {
    v.push(d.len() as u8);
    v.extend_from_slice(d);
}

#[tokio::test]
async fn regtest_sign_e2e() {
    let seed_hex =
        std::env::var("LTC_TESTNET_SEED").expect("Set LTC_TESTNET_SEED=0x... to run this test");
    let seed_hex = seed_hex.strip_prefix("0x").unwrap_or(&seed_hex);
    let seed = hex::decode(seed_hex).expect("seed must be hex");
    let prev_txid_hex = std::env::var("LTC_PREV_TXID").expect("Set LTC_PREV_TXID=<funding txid>");
    let prev_vout: u32 = std::env::var("LTC_PREV_VOUT")
        .expect("Set LTC_PREV_VOUT=<vout>")
        .parse()
        .expect("vout must be u32");
    let prev_value: u64 = std::env::var("LTC_PREV_VALUE")
        .expect("Set LTC_PREV_VALUE=<satoshis>")
        .parse()
        .expect("value must be u64");

    let plugin = LtcPlugin::new();
    let acct = plugin
        .create_account(&seed, 0, "litecoin-testnet")
        .await
        .expect("create_account must succeed");
    let addr = acct.address.clone();
    eprintln!("Derived address: {addr}");
    assert!(addr.starts_with('m') || addr.starts_with('n'), "expected P2PKH, got {addr}");

    let script_pubkey = p2pkh_script_from_address(&addr).expect("recompute P2PKH script");
    assert!(script_pubkey.is_p2pkh(), "expected P2PKH script");

    let prev_txid = prev_txid_hex.parse::<bitcoin::Txid>().expect("txid valid");
    eprintln!("Spending prev {prev_txid}:{prev_vout} = {prev_value} sats");

    // Build a PSBT spending the P2PKH UTXO (fee 1000 sats)
    let fee = 1000u64;
    let send_amount = prev_value - fee;
    let unsigned_tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::new(prev_txid, prev_vout),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(send_amount),
            // burn to a zero P2PKH (regtest)
            script_pubkey: ScriptBuf::from_bytes(vec![
                0x76, 0xa9, 0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88,
                0xac,
            ]),
        }],
    };

    let psbt = Psbt {
        unsigned_tx,
        version: 0,
        xpub: Default::default(),
        proprietary: Default::default(),
        unknown: Default::default(),
        inputs: vec![bitcoin::psbt::Input {
            witness_utxo: Some(TxOut {
                value: Amount::from_sat(prev_value),
                script_pubkey,
            }),
            ..Default::default()
        }],
        outputs: vec![Default::default()],
    };

    let psbt_bytes = psbt.serialize();
    let signed_bytes = plugin
        .sign_transaction(&psbt_bytes, &seed, 0, "litecoin-testnet")
        .await
        .expect("sign_transaction should succeed (P2PKH legacy path)");
    let signed_psbt = Psbt::deserialize(&signed_bytes).expect("signed psbt deserialize");
    assert!(
        !signed_psbt.inputs[0].partial_sigs.is_empty(),
        "should have a partial sig (P2PKH signing)"
    );

    // Finalize: scriptSig = <sig> <pubkey>, empty witness
    let (pk, sig) = signed_psbt.inputs[0]
        .partial_sigs
        .iter()
        .next()
        .expect("partial sig");
    let mut script_sig_bytes = Vec::new();
    push_cs(&mut script_sig_bytes, &sig.to_vec());
    push_cs(&mut script_sig_bytes, &pk.to_bytes());

    let signed_tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::new(prev_txid, prev_vout),
            script_sig: ScriptBuf::from_bytes(script_sig_bytes),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(send_amount),
            script_pubkey: signed_psbt.unsigned_tx.output[0].script_pubkey.clone(),
        }],
    };
    let signed_tx_bytes = bitcoin::consensus::serialize(&signed_tx);
    let signed_tx_hex = hex::encode(&signed_tx_bytes);
    eprintln!("Signed tx ({} bytes) ready", signed_tx_bytes.len());
    println!("LTC_SIGNED_TX_HEX={signed_tx_hex}");
}