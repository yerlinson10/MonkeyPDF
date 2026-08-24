use std::path::Path;

fn main() {
    let engine = Path::new("engine.exe");
    if !engine.exists() {
        std::fs::write(engine, []).expect("stub engine.exe");
    }
    println!("cargo:rerun-if-changed=engine.exe");
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-changed=ui/uninstall.html");
    println!("cargo:rerun-if-changed=ui/logo.svg");
    println!("cargo:rerun-if-changed=../src-tauri/icons/icon.ico");
    println!("cargo:rerun-if-changed=../src-tauri/icons/32x32.png");

    let ico = Path::new("../src-tauri/icons/icon.ico");
    if ico.exists() {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../src-tauri/icons/icon.ico");
        res.set("ProductName", "MonkeyPDF");
        res.set("FileDescription", "MonkeyPDF Setup");
        if let Err(e) = res.compile() {
            println!("cargo:warning=winres: {e}");
        }
    }
}
