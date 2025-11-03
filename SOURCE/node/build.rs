const ENVIRONMENT_VARIABLES: &[&str] = &["NETWORK_NAME", "NETWORK_PASS", "NETWORK_PORT"];

fn main() {
    linker_be_nice();
    watch_env_variables();
    println!("cargo:rustc-link-arg=-Tlinkall.x");
}

fn linker_be_nice() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let kind = &args[1];
        let what = &args[2];

        match kind.as_str() {
            "undefined-symbol" => match what.as_str() {
                "_defmt_timestamp" => {
                    eprintln!();
                    eprintln!(
                        "💡 `defmt` not found - make sure `defmt.x` is added as a linker script and you have included `use defmt_rtt as _;`"
                    );
                    eprintln!();
                }
                "_stack_start" => {
                    eprintln!();
                    eprintln!("💡 Is the linker script `linkall.x` missing?");
                    eprintln!();
                }
                "esp_rtos_initialized" | "esp_rtos_yield_task" | "esp_rtos_task_create" => {
                    eprintln!();
                    eprintln!(
                        "💡 `esp-radio` has no scheduler enabled. Make sure you have initialized `esp-rtos` or provided an external scheduler."
                    );
                    eprintln!();
                }
                "embedded_test_linker_file_not_added_to_rustflags" => {
                    eprintln!();
                    eprintln!(
                        "💡 `embedded-test` not found - make sure `embedded-test.x` is added as a linker script for tests."
                    );
                    eprintln!();
                }
                _ => (),
            },
            _ => {
                std::process::exit(1);
            }
        }

        std::process::exit(0);
    }

    println!(
        "cargo:rustc-link-arg=--error-handling-script={}",
        std::env::current_exe()
            .expect("cargo executable should exist in a cargo build script")
            .display()
    );
}

fn watch_env_variables() {
    for variable in ENVIRONMENT_VARIABLES {
        println!("cargo:rerun-if-changed={variable}");
    }

    let name = std::env::var("NETWORK_NAME").unwrap_or_else(|_| "__raftor".to_string());
    let pass = std::env::var("NETWORK_PASS").unwrap_or_else(|_| "__raftor".to_string());
    let port = std::env::var("NETWORK_PORT").unwrap_or_else(|_| "9999".to_string());
    let content = format!(
        "pub const NETWORK_NAME: &str = {name:?};\n\
         pub const NETWORK_PASS: &str = {pass:?};\n\
         pub const NETWORK_PORT: u16 = {port};",
    );

    let output = std::env::var_os("OUT_DIR").expect("output directory should exist");
    let destination = std::path::Path::new(&output).join("config.rs");
    std::fs::write(&destination, content).expect("writing valid Rust code should success");
}
