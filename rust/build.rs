use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=tools/codegen.py");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Failed to get CARGO_MANIFEST_DIR");
    let project_root = PathBuf::from(manifest_dir);
    // println!("cargo:warning=Project root: {:?}", project_root);

    let generator_path = project_root.join("generator");
    let codegen_script_path = project_root.join("tools/codegen.py");

    let config = pyo3_build_config::get();
    let python_executable_path = &config.executable().unwrap();

    println!("cargo:warning=Using Python: {:?}", python_executable_path);

    let status = Command::new(python_executable_path)
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("-e")
        .arg(generator_path)
        .status()
        .expect("Failed to execute pip install generator");

    if !status.success() {
        panic!("install telethon_generator failed with status {:?}", status);
    }

    let status = Command::new(python_executable_path)
        .arg(codegen_script_path)
        .status()
        .expect("Failed to execute tools/codegen.py");

    if !status.success() {
        panic!("codegen.py failed with status {:?}", status);
    }
}
