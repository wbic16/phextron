//! Windows round-trip: store -> read back -> compare -> delete.
//! Test target is clearly named so it can never collide with real credentials.
use gl2_credman::{delete, read, store};

fn main() {
    let target = "phextron-gl2-test/roundtrip";
    let blob = b"phextron-gl2-roundtrip-test-blob";
    store(target, blob).expect("store failed");
    println!("stored {} bytes under {target}", blob.len());
    let back = read(target).expect("read failed").expect("credential missing after store");
    assert_eq!(back, blob.to_vec(), "round-trip blob mismatch");
    println!("read back {} bytes, match OK", back.len());
    delete(target).expect("delete failed");
    let after = read(target).expect("read-after-delete failed");
    assert!(after.is_none(), "credential still present after delete");
    println!("deleted; read-after-delete = None");
    println!("CREDMAN ROUNDTRIP PASS");
}
