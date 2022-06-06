#[cfg(windows)]
fn main() {
    use std::env;

    let mut res = winres::WindowsResource::new();
    
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    if let Ok(sha) = env::var("GITHUB_SHA") {
        let version = env::var("CARGO_PKG_VERSION").unwrap();
        res.set("ProductVersion", &format!("{version}+{sha}"));
    }

    res.compile().unwrap();
}

#[cfg(unix)]
    fn main() {
}
