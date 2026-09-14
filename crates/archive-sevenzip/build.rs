fn main() {
    println!("cargo:rerun-if-env-changed=QZIP_MAC_ENGINE_SHA256");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let hash = std::env::var("QZIP_MAC_ENGINE_SHA256")
            .expect("Run scripts/macos.mjs fetch before building; source its engine.env in CI");
        assert!(hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()));
        println!("cargo:rustc-env=QZIP_MAC_ENGINE_SHA256={hash}");
    }
}
