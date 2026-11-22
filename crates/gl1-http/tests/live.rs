use gl1_http::LatticeClient;

/// Env-gated live test: only runs when GL1_LIVE=1.
#[test]
fn live_session() {
    if std::env::var("GL1_LIVE").ok().as_deref() != Some("1") {
        eprintln!("skipping live test (set GL1_LIVE=1)");
        return;
    }
    let client = LatticeClient::new("https://edit.phext.io");
    let (status, body) = client.session().expect("live GET /api/session");
    println!("live status={status} body={body}");
    assert_eq!(status, 200);
    assert!(body.contains("\"signed_in\""), "body must contain signed_in: {body}");
}
