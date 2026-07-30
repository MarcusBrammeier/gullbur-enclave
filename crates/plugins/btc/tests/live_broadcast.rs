//! E2E Bitcoin testnet broadcast test.
//!
//! Requires BTC_TESTNET_SEED env var (hex seed for BIP-84 key derivation).
//! The address must have at least one confirmed UTXO.
//!
//! Run: BTC_TESTNET_SEED=0x... cargo test -p plugin-btc --test live_broadcast -- --ignored

use plugin_btc::BtcPlugin;
use wallet_plugin::{KeyHandle, KeyType, WalletPlugin};

const RPC: &str = "https://blockstream.info/testnet/api";

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build().expect("test invariant")
}

async fn esplora_get_json(path: &str) -> serde_json::Value {
    let url = format!("{RPC}/{path}");
    let resp = build_client().get(&url).send().await.expect("esplora GET failed");
    resp.json().await.expect("esplora JSON parse failed")
}

async fn esplora_get_bytes(path: &str) -> Vec<u8> {
    let url = format!("{RPC}/{path}");
    let resp = build_client().get(&url).send().await.expect("esplora GET failed");
    resp.bytes().await.expect("esplora bytes GET failed").to_vec()
}

async fn esplora_post(path: &str, body: &str) -> String {
    let url = format!("{RPC}/{path}");
    let resp = build_client()
        .post(&url)
        .header("Content-Type", "text/plain")
        .body(body.to_string())
        .send().await.expect("esplora POST failed");
    resp.text().await.expect("esplora response failed")
}

#[tokio::test]
#[ignore = "requires BTC_TESTNET_SEED env var and testnet BTC"]
async fn live_broadcast_testnet() {
    let seed_hex = std::env::var("BTC_TESTNET_SEED")
        .expect("Set BTC_TESTNET_SEED=0x... to run this test");
    let seed_hex = seed_hex.strip_prefix("0x").unwrap_or(&seed_hex);
    let seed = hex::decode(seed_hex).expect("Seed must be hex");

    let plugin = BtcPlugin::new(None);
    let account = plugin.create_account(&seed, 0, "bitcoin-testnet").await.expect("test invariant");
    eprintln!("Address: {}", account.address);

    // Wait for confirmed UTXO (poll up to 120s)
    let start = std::time::Instant::now();
    let max_wait = std::time::Duration::from_secs(120);
    let utxo = loop {
        let utxos = esplora_get_json(&format!("address/{}/utxo", account.address)).await;
        let utxos: Vec<serde_json::Value> = serde_json::from_value(utxos).expect("utxos should be an array");
        if let Some(u) = utxos.iter().find(|u| u["status"]["confirmed"].as_bool().unwrap_or(false)) {
            break u.clone();
        }
        if start.elapsed() > max_wait {
            panic!("Timeout waiting for confirmed UTXO (120s)");
        }
        eprintln!("Waiting for confirmation...");
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    };

    let txid = utxo["txid"].as_str().expect("test invariant");
    let vout = utxo["vout"].as_u64().expect("test invariant") as u32;
    let value = utxo["value"].as_u64().expect("test invariant");
    let fee = 1000u64; // 1000 sats
    let send_amount = value - fee;
    eprintln!("UTXO: {txid}:{vout} = {value} sats");
    eprintln!("Sending: {send_amount} sats (fee: {fee})");

    // Fetch the raw transaction for the UTXO to get the scriptPubKey
    let raw_tx_bytes = esplora_get_bytes(&format!("tx/{txid}/raw")).await;
    let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&raw_tx_bytes).expect("test invariant");
    let script_pubkey = tx.output[vout as usize].script_pubkey.clone();

    // Build a simple P2WPKH PSBT
    use bitcoin::absolute::LockTime;
    use bitcoin::psbt::Psbt;
    use bitcoin::transaction::Version;
    use bitcoin::{Amount, OutPoint, Sequence, TxIn, TxOut, Witness};

    // Burn address: tb1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3uc9uu
    let dest_script = bitcoin::ScriptBuf::from_bytes(vec![0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

    let unsigned_tx = bitcoin::Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::new(txid.parse().expect("test invariant"), vout),
            script_sig: bitcoin::ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![
            TxOut { value: Amount::from_sat(send_amount), script_pubkey: dest_script },
        ],
    };

    let psbt = Psbt {
        unsigned_tx,
        version: 0,
        xpub: Default::default(),
        proprietary: Default::default(),
        unknown: Default::default(),
        inputs: vec![bitcoin::psbt::Input {
            witness_utxo: Some(TxOut {
                value: Amount::from_sat(value),
                script_pubkey,
            }),
            ..Default::default()
        }],
        outputs: vec![Default::default()],
    };

    let psbt_bytes = psbt.serialize();
    let key = KeyHandle {
        key_id: seed_hex.to_string(),
        key_type: KeyType::Secp256k1,
        public_key: vec![],
    };

    let signed_psbt_bytes = plugin.sign_transaction(&psbt_bytes, &key, "bitcoin-testnet").await
        .expect("sign_transaction should succeed");
    let signed_psbt = Psbt::deserialize(&signed_psbt_bytes)
        .expect("signed PSBT should deserialize");
    assert!(!signed_psbt.inputs[0].partial_sigs.is_empty(), "should have a signature");

    // Manually construct the final transaction with witness from PSBT
    let (pk, sig) = signed_psbt.inputs[0].partial_sigs.iter().next()
        .expect("should have partial sigs");
    let mut witness = Witness::new();
    witness.push(&sig.to_vec());
    witness.push(&pk.to_bytes());

    let signed_tx = bitcoin::Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![bitcoin::TxIn {
            previous_output: bitcoin::OutPoint::new(txid.parse().expect("test invariant"), vout),
            script_sig: bitcoin::ScriptBuf::new(),
            sequence: bitcoin::Sequence::MAX,
            witness,
        }],
        output: vec![
            bitcoin::TxOut { value: bitcoin::Amount::from_sat(send_amount), script_pubkey: signed_psbt.unsigned_tx.output[0].script_pubkey.clone() },
        ],
    };
    let signed_tx_bytes = bitcoin::consensus::serialize(&signed_tx);
    let signed_tx_hex = hex::encode(&signed_tx_bytes);

    eprintln!("Broadcasting {} bytes...", signed_tx_bytes.len());
    let txid_result = esplora_post("tx", &signed_tx_hex).await.trim().to_string();
    eprintln!("Broadcast result: {txid_result}");

    // Verify txid is 64 hex chars
    assert_eq!(txid_result.len(), 64, "Should return a valid txid");

    // Poll for confirmation (optional — testnet3 may be slow)
    eprintln!("\n⏳ Waiting up to 600s for confirmation (polling every 15s)...");
    let start = std::time::Instant::now();
    let max_wait = std::time::Duration::from_secs(600);
    let mut confirmed = false;
    loop {
        let status: serde_json::Value = esplora_get_json(&format!("tx/{txid_result}/status")).await;
        if status["confirmed"].as_bool().unwrap_or(false) {
            let block = status["block_height"].as_u64().unwrap_or(0);
            eprintln!("\n🎉 Transaction confirmed in block #{block}!");
            eprintln!("   TxID: {txid_result}");
            confirmed = true;
            break;
        }
        if start.elapsed() > max_wait {
            eprintln!("\n⚠️  Not yet confirmed after 600s (testnet3 may be slow)");
            eprintln!("   TxID still in mempool: {txid_result}");
            break;
        }
        eprintln!("  Waiting for confirmation... ({:.0}s elapsed)", start.elapsed().as_secs_f64());
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    }
    // Broadcast success is the critical assertion — confirmation is a bonus
    if !confirmed {
        eprintln!("   Test passing — broadcast verified, confirmation pending on testnet");
    }
}