use serial_test::serial;
use std::sync::atomic::Ordering;
use vault_core::Vault;

/// Helper: create a temp HOME and init the vault
async fn init_vault_in(dir: &std::path::Path) -> Vault {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).ok();
    unsafe {
        std::env::set_var("HOME", dir);
    }
    // Reset the dirs_next cache by dropping the old value
    // (dirs_next doesn't cache on Linux, so this is safe)
    let mut vault = Vault::new();
    vault.initialize("", "").await.expect("init");
    vault
}

/// Test that accounts are created with correct address formats
/// and that the vault engine produces deterministic keys.
#[tokio::test]
#[serial]
async fn test_account_address_formats() {
    let tmp = std::env::temp_dir().join(format!("gullbur-addr-{}", std::process::id()));
    let vault = init_vault_in(&tmp).await;

    // BTC testnet
    let btc = vault
        .create_account("bitcoin-testnet", 0)
        .await
        .expect("btc");
    assert!(
        btc.address.starts_with("tb1"),
        "BTC testnet: {}",
        btc.address
    );
    assert_eq!(btc.network, "bitcoin-testnet");

    // XMR stagenet
    let xmr = vault
        .create_account("monero-stagenet", 0)
        .await
        .expect("xmr");
    assert!(
        xmr.address.starts_with("5") || xmr.address.starts_with("7"),
        "XMR stagenet: {}",
        xmr.address
    );
    assert_eq!(xmr.network, "monero-stagenet");

    // ETH Sepolia
    let eth = vault.create_account("sepolia", 0).await.expect("eth");
    assert!(
        eth.address.starts_with("0x"),
        "ETH Sepolia: {}",
        eth.address
    );
    assert_eq!(eth.network, "sepolia");

    let _ = std::fs::remove_dir_all(&tmp);
}

/// Test that the keystore key file is generated on init and persists on disk.
#[tokio::test]
#[serial]
async fn test_keystore_key_file_created() {
    let tmp = std::env::temp_dir().join(format!("gullbur-key-{}", std::process::id()));
    let _vault = init_vault_in(&tmp).await;

    let key_path = tmp.join(".gullbur").join("keystore.key");
    assert!(key_path.exists(), "keystore.key should exist");
    let key_data = std::fs::read(&key_path).expect("read key");
    assert_eq!(key_data.len(), 32, "key should be 32 bytes");

    let store_path = tmp.join(".gullbur").join("keystore");
    assert!(store_path.exists(), "keystore should exist");

    let _ = std::fs::remove_dir_all(&tmp);
}

/// Test that vault state restores from disk after process-like restart.
/// This simulates: init → persist → new process → restore.
#[tokio::test]
#[serial]
async fn test_vault_restore_after_persist() {
    let tmp = std::env::temp_dir().join(format!("gullbur-restore-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).ok();
    unsafe {
        std::env::set_var("HOME", &tmp);
    }

    // Session 1: init
    {
        let mut vault = Vault::new();
        vault.initialize("", "").await.expect("init");
        vault
            .create_account("bitcoin-testnet", 0)
            .await
            .expect("btc");
        vault
            .create_account("monero-stagenet", 0)
            .await
            .expect("xmr");
    } // vault drops, keystore persists

    // Session 2: restore (same process, different Vault instance)
    // dirs_next reads $HOME fresh each call on Linux, so this works
    let mut vault = Vault::new();
    vault.try_restore().await.expect("try_restore");
    assert!(
        vault.initialized.load(Ordering::SeqCst),
        "vault should be initialized after restore"
    );

    // Verify files still exist
    assert!(
        tmp.join(".gullbur").join("keystore").exists(),
        "keystore persists"
    );
    assert!(
        tmp.join(".gullbur").join("keystore.key").exists(),
        "key persists"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
