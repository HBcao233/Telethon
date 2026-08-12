use std::env;
use std::path::PathBuf;
use std::process::Command;

fn get_python_executable() -> PathBuf {
    if let Ok(p) = env::var("PYO3_PYTHON") {
        return PathBuf::from(p);
    }
    match pyo3_build_config::get().executable() {
        Some(path) => PathBuf::from(path),
        None => {
            env::var("PYTHON_EXECUTABLE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("python3"))
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=tools/codegen.py");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Failed to get CARGO_MANIFEST_DIR");
    let project_root = PathBuf::from(manifest_dir);
    // println!("cargo:warning=Project root: {:?}", project_root);

    let generator_path = project_root.join("generator");
    let codegen_script_path = project_root.join("tools/codegen.py");

    let python_executable_path = get_python_executable();

    println!("cargo:warning=Using Python: {:?}", python_executable_path);

    let status = Command::new(&python_executable_path)
        .arg("-m")
        .arg("pip")
        .arg("install")
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
