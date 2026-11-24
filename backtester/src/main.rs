//! CLI: `oculus-backtest backtest --config config/params.yaml --data sample/`

use std::fs;

use oculus_backtester::{load_bars, run};
use oculus_config_rs::load as load_params;

fn main() {
    if let Err(error) = real_main() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "backtest") {
        args.remove(0);
    }
    let mut config = "config/params.yaml".to_owned();
    let mut data = "sample/".to_owned();
    let mut out: Option<String> = None;

    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                index += 1;
                config = args.get(index).cloned().ok_or("--config needs a path")?;
            }
            "--data" => {
                index += 1;
                data = args.get(index).cloned().ok_or("--data needs a path")?;
            }
            "--out" => {
                index += 1;
                out = Some(args.get(index).cloned().ok_or("--out needs a path")?);
            }
            "--help" | "-h" => {
                println!("usage: oculus-backtest [backtest] --config <params.yaml> --data <dir-or-csv> [--out <file>]");
                return Ok(());
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
        index += 1;
    }

    let params = load_params(&config).map_err(|e| e.to_string())?;
    let bars = load_bars(&data)?;
    let report = run(&params, &bars)?;
    let text = report.to_string();
    print!("{text}");
    if let Some(path) = out {
        fs::write(&path, &text).map_err(|e| format!("cannot write {path}: {e}"))?;
    }
    Ok(())
}
