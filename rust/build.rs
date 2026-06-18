use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=tools/codegen.py");

    let config = pyo3_build_config::get();
    let python_executable_path = &config.executable.clone().unwrap();

    println!("cargo:warning=Using Python: {:?}", python_executable_path);

    let status = Command::new(python_executable_path)
        .arg("tools/codegen.py")
        .status()
        .expect("Failed to execute tools/codegen.py");

    if !status.success() {
        panic!("codegen.py failed with status {:?}", status);
    }
}
