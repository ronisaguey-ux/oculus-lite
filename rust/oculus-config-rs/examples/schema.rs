//! Print the parameter reference table. Used to regenerate docs/PARAMETERS.md.
//! `cargo run -p oculus-config-rs --example schema`

fn main() {
    print!("{}", oculus_config_rs::schema_markdown());
}
