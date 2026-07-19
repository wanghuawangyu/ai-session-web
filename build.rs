fn main() {
    // 允许在 CI 中通过 RELEASE_VERSION 环境变量覆盖版本号
    // 例如：RELEASE_VERSION=v0.1.0 cargo build --release
    if let Ok(tag) = std::env::var("RELEASE_VERSION") {
        let tag = tag.trim_start_matches('v');
        println!("cargo:rustc-env=CARGO_PKG_VERSION={}", tag);
    }
}
