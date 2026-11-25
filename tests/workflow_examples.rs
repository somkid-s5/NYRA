use std::{path::PathBuf, process::Command};

#[test]
fn bundled_custom_workflows_pass_the_cli_validator() {
    let examples = [
        "local-system-health.toml",
        "certificate-inspection.toml",
        "database-port-check.toml",
        "network-diagnostics.toml",
    ];

    for example in examples {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join("workflows")
            .join(example);
        let output = Command::new(env!("CARGO_BIN_EXE_nyra"))
            .args(["workflow", "validate"])
            .arg(path)
            .output()
            .expect("run NYRA workflow validator");

        assert_eq!(
            output.status.code(),
            Some(0),
            "bundled workflow {example} did not validate"
        );
        assert!(
            output.stderr.is_empty(),
            "workflow {example} wrote to stderr"
        );
    }
}
