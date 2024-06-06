use std::process::Command;

use serial_test::serial;

#[test]
#[serial]
fn test_instance_creation() {
    let _ = Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("instance_creation")
        .spawn()
        .expect("Failed to execute command")
        .wait();
}
