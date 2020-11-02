use std::process::Command;

fn main() {
    if std::env::var("XARGO").is_err() {
        println!("cargo:warning=(not a warning) Building BPF shared-memory program");
        Command::new("cargo")
            .arg("build-bpf")
            .status()
            .expect("Failed to build BPF shared-memory program")
            .success();
    }
}
