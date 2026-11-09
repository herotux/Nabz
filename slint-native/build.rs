use std::{env, path::PathBuf, process::Command};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().expect("slint-native must be inside the repository");
    let font = repo_root.join("assets/Vazirmatn.ttf");

    if !font.is_file() {
        let status = Command::new("bash")
            .arg(repo_root.join("scripts/fetch-font.sh"))
            .current_dir(repo_root)
            .status()
            .expect("failed to run scripts/fetch-font.sh");
        assert!(status.success(), "failed to download the bundled Vazirmatn font");
    }

    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=../assets/Vazirmatn.ttf");
    println!("cargo:rerun-if-changed=../scripts/fetch-font.sh");
    slint_build::compile("ui/main.slint").expect("failed to compile Slint UI");
}
