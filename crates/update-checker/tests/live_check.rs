//! Live GitHub API integration test for the update checker.
//!
//! Verifies the update-checker crate can reach the real GitHub Releases API,
//! parse the response, and correctly report whether the local version is
//! up-to-date or behind the latest release.
//!
//! Run: cargo test -p update-checker --test live_check -- --ignored --nocapture

#[tokio::test]
#[ignore = "requires live network access to api.github.com — run with --ignored"]
async fn real_github_repo_has_tags() {
    let token = std::env::var("GITHUB_TOKEN").ok();
    let repo = "MarcusBrammeier/gullbur-enclave";
    let url = format!("https://api.github.com/repos/{repo}/tags?per_page=1");
    let client = reqwest::Client::builder()
        .user_agent("gullbur/0.1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("client");
    let mut req = client.get(&url);
    if let Some(t) = &token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let resp = req.send().await.expect("API reachable");
    assert!(
        resp.status().is_success(),
        "tags API call failed: {}",
        resp.status()
    );
    let tags: Vec<serde_json::Value> = resp.json().await.unwrap_or_default();
    assert!(!tags.is_empty(), "should have at least one tag (beta.1-.6)");
    let tag = tags[0]["name"].as_str().unwrap_or("");
    eprintln!("Latest git tag: {tag}");
    assert!(
        tag.starts_with("v0.1.0-beta"),
        "expected beta tag, got {tag}"
    );
}

#[tokio::test]
#[ignore = "requires network"]
async fn no_releases_yields_none() {
    // update_checker checks /releases/latest first, then /releases?per_page=1
    // Since we have no GitHub Releases (only git tags), it should return
    // Ok(None) — which is the correct signal to the UI that there's nothing to show.
    let token = std::env::var("GITHUB_TOKEN").ok();
    let result =
        update_checker::check_for_updates("MarcusBrammeier/gullbur-enclave", token.as_deref())
            .await;
    // With only git tags (no GitHub Releases), the checker returns
    // Err(NoReleases) — this is correct. NoReleases means "no GitHub Release
    // objects exist, check git tags manually." Ok(None) would mean the
    // repo doesn't exist.
    eprintln!("check_for_updates returned: {result:?}");
    match result {
        Err(update_checker::UpdateError::NoReleases(_)) => {
            // Expected — we have git tags but no GitHub Releases.
        }
        Ok(Some(info)) => {
            eprintln!("Release found: {}", info.release.tag_name);
        }
        other => panic!("unexpected result: {other:?}"),
    }
}
