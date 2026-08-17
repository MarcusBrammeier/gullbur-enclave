//! LTC testnet3 live broadcast test.
//!
//! Spends a P2PKH UTXO at the derived index-0 address (mkenEChN3CvkNr2hKxUkT72phJ5cZwGgXT).
//! The outpoint (prev txid + vout), value, and prevout P2PKH script are derived from
//! env vars (the prevout script is recomputed from the address, so no raw funding hex
//! is required — bitaps is Cloudflare-gated and mempool.space 404s the tx).
//!
//! Requires:
//!   LTC_TESTNET_SEED   - hex seed (BIP-84 / m/84'/2'/0'/0/0 derivation)
//!   LTC_PREV_TXID      - outgoing prev txid hex (funding tx that paid our address)
//!   LTC_PREV_VOUT      - output index of the P2PKH output paying our address
//!   LTC_PREV_VALUE     - value of that output in satoshis
//!   (optional) LTC_BROADCAST_API - esplora base host for broadcast
//!
//! Run:
//!   LTC_TESTNET_SEED=0x... LTC_PREV_TXID=<txid> LTC_PREV_VOUT=0 LTC_PREV_VALUE=1000000 \
//!     cargo test -p plugin-ltc --test live_broadcast -- --ignored --nocapture

use bitcoin::absolute::LockTime;
use bitcoin::psbt::Psbt;
use bitcoin::transaction::Version;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness};
use plugin_ltc::LtcPlugin;
use wallet_plugin::WalletPlugin;

async fn broadcast_tx(hex: &str, base: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let url = format!("{base}/tx");
    let resp = client
        .post(&url)
        .header("Content-Type", "text/plain")
        .body(hex.to_string())
        .send()
        .await
        .map_err(|e| format!("broadcast HTTP: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("read resp: {e}"))?;
    let txid = body.trim().to_string();
    if !status.is_success() || txid.len() != 64 || txid.chars().any(|c| !c.is_ascii_hexdigit()) {
        return Err(format!("broadcast rejected ({status}): {txid}"));
    }
    Ok(txid)
}

/// Recompute a P2PKH script_pubkey from a legacy m/n address (version 0x6f).
fn p2pkh_script_from_address(addr: &str) -> Result<ScriptBuf, String> {
    let decoded = base58ck::decode_check(addr).map_err(|e| format!("base58 decode: {e}"))?;
    if decoded.len() != 21 || decoded[0] != 0x6f {
        return Err(format!("not a valid testnet P2PKH address (len {} ver {}): {addr}", decoded.len(), decoded.get(0).map(|b| *b).unwrap_or(0)));
    }
    let mut script = vec![0x76, 0xa9, 0x14]; // OP_DUP OP_HASH160 PUSH20
    script.extend_from_slice(&decoded[1..21]);
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG
    Ok(ScriptBuf::from_bytes(script))
}

/// Pushes a length-prefixed byte vector (CScript push, single-byte length).
fn push_cs(v: &mut Vec<u8>, d: &[u8]) {
    v.push(d.len() as u8);
    v.extend_from_slice(d);
}

#[tokio::test]
#[ignore = "requires LTC_TESTNET_SEED and a funded testnet3 UTXO"]
async fn live_broadcast_testnet3() {
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
    let broadcast_base = std::env::var("LTC_BROADCAST_API")
        .unwrap_or_else(|_| "https://litecoin.mempool.space/testnet/api".to_string());

    let plugin = LtcPlugin::new();
    let acct = plugin
        .create_account(&seed, 0, "litecoin-testnet")
        .await
        .expect("derive address");
    let addr = acct.address.clone();
    eprintln!("Derived address: {addr}");
    assert!(
        addr.starts_with('m') || addr.starts_with('n'),
        "expected legacy testnet P2PKH address, got {addr}"
    );

    let script_pubkey = p2pkh_script_from_address(&addr).expect("recompute P2PKH script");
    assert!(script_pubkey.is_p2pkh(), "expected P2PKH script");

    let prev_txid = prev_txid_hex
        .parse::<bitcoin::Txid>()
        .expect("prev txid must be 64-hex");
    eprintln!(
        "Spending prev {prev_txid}:{prev_vout} = {prev_value} sats (testnet3 P2PKH)"
    );

    // Build a PSBT spending the P2PKH UTXO
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
            // burn to a zero P2PKH (testnet)
            script_pubkey: ScriptBuf::from_bytes(vec![
                0x76, 0xa9, 0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x88, 0xac,
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
        // pass DECODED seed bytes so plugin derives the SAME key as create_account
        .sign_transaction(&psbt_bytes, &seed, 0, "litecoin-testnet")
        .await
        .expect("sign_transaction should succeed (P2PKH legacy path)");
    let signed_psbt = Psbt::deserialize(&signed_bytes).expect("signed psbt deserialize");
    assert!(
        !signed_psbt.inputs[0].partial_sigs.is_empty(),
        "should have a partial sig (P2PKH signing)"
    );

    // Legacy P2PKH finalization: scriptSig = <sig> <pubkey>, empty witness
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
            sequence: bitcoin::Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: bitcoin::Amount::from_sat(send_amount),
            script_pubkey: signed_psbt.unsigned_tx.output[0].script_pubkey.clone(),
        }],
    };
    let signed_tx_bytes = bitcoin::consensus::serialize(&signed_tx);
    let signed_tx_hex = hex::encode(&signed_tx_bytes);
    eprintln!("Signed tx ({} bytes) ready", signed_tx_bytes.len());
    eprintln!("LTC_SIGNED_TX_HEX={signed_tx_hex}");

    eprintln!("Broadcasting to {broadcast_base}...");
    match broadcast_tx(&signed_tx_hex, &broadcast_base).await {
        Ok(broadcast_txid) => {
            eprintln!("\n🎉 LTC testnet3 broadcast SUCCESS");
            eprintln!("   TxID: {broadcast_txid}");
        }
        Err(e) => {
            eprintln!("\n❌ Broadcast FAILED: {e}");
            panic!("broadcast failed: {e}");
        }
    }
}