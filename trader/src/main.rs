//! CLI: `oculus-trader paper --config config/params.yaml --data sample/`
//! or `oculus-trader --live --config config/params.yaml --data sample/`

use std::env;

use oculus_backtester::load_bars;
use oculus_config_rs::load as load_params;
use oculus_trader::{live_credentials, run_live, run_paper};

fn main() {
    let live = env::args().any(|arg| arg == "--live");
    if let Err(error) = real_main() {
        eprintln!("error: {error}");
        std::process::exit(if live { 2 } else { 1 });
    }
}

fn real_main() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    args.retain(|arg| arg != "--live");
    if args.first().is_some_and(|a| a == "paper" || a == "live") {
        args.remove(0);
    }
    let mut config = "config/params.yaml".to_owned();
    let mut data = "sample/".to_owned();
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
            "--help" | "-h" => {
                println!("usage: oculus-trader [paper] [--live] --config <params.yaml> --data <dir-or-csv>");
                return Ok(());
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
        index += 1;
    }

    let params = load_params(&config).map_err(|e| e.to_string())?;
    let bars = load_bars(&data)?;

    if env::args().any(|arg| arg == "--live") {
        let key = env::var("OCULUS_API_KEY").ok();
        let secret = env::var("OCULUS_API_SECRET").ok();
        let url = env::var("OCULUS_LIVE_URL").ok();
        let credentials = live_credentials(key.as_deref(), secret.as_deref(), url.as_deref())?;
        let accepted = run_live(&params, credentials, &bars)?;
        println!("mode=live venue=operator-supplied orders_accepted={}", accepted.len());
        println!("live fills are venue-reported; this lite edition does not simulate them");
    } else {
        let report = run_paper(&params, &bars)?;
        println!("mode=paper (default; no credentials and no network access)");
        print!("{report}");
    }
    Ok(())
}
