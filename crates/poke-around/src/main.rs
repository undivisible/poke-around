use poke_around::{Error, Result, agents, bridge, config, daemon};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let verbose = has_flag(&args, "--verbose") || has_flag(&args, "-v");
    let mode = flag_value(&args, "--mode");

    if has_flag(&args, "--version") || has_flag(&args, "-V") {
        println!("poke-around {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.is_empty()
        || has_flag(&args, "--foreground")
        || args.first().is_some_and(|arg| arg == "daemon")
    {
        return daemon::run(mode.as_deref(), verbose);
    }

    match args[0].as_str() {
        "run-agent" => {
            let name = args
                .get(1)
                .ok_or_else(|| Error::msg("Usage: poke-around run-agent <name>"))?;
            agents::run_agent_by_name(name)
        }
        "agent" => run_agent_command(&args[1..]),
        "take-screenshot" => {
            let capture =
                rs_peekaboo::Peekaboo::new().image(rs_peekaboo::ImageMode::Screen, None, true)?;
            println!("{}", capture.path.display());
            Ok(())
        }
        "notify" => {
            let message = args
                .get(1)
                .map(String::as_str)
                .unwrap_or("Hello from Poke Around");
            bridge::send_one_shot_message(message)
        }
        "set-mode" => {
            let mode = args
                .get(1)
                .ok_or_else(|| Error::msg("Usage: poke-around set-mode <full|limited|sandbox>"))?;
            if !matches!(mode.as_str(), "full" | "limited" | "sandbox") {
                return Err(Error::msg("invalid mode"));
            }
            config::save_permission_mode(mode)?;
            println!("Permission mode set to: {mode}");
            Ok(())
        }
        "status" => {
            println!("poke-around {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "--help" | "-h" | "help" => {
            print_help();
            Ok(())
        }
        other => Err(Error::msg(format!(
            "Unknown command: {other}\nRun 'poke-around --help' for usage."
        ))),
    }
}

fn run_agent_command(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("get") => {
            let name = args
                .get(1)
                .ok_or_else(|| Error::msg("Usage: poke-around agent get <name>"))?;
            let path = agents::download_agent(name)?;
            println!("{}", path.display());
            Ok(())
        }
        Some("create") => {
            let prompt = flag_value(args, "--prompt").or_else(|| args.get(1).cloned());
            let path = agents::create_agent(prompt.as_deref())?;
            println!("{}", path.display());
            Ok(())
        }
        _ => Err(Error::msg("Usage: poke-around agent <get|create>")),
    }
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn print_help() {
    println!(
        "Usage: poke-around [--verbose] [--mode full|limited|sandbox]\n       poke-around run-agent <name>\n       poke-around agent get <name>\n       poke-around agent create [--prompt text]\n       poke-around take-screenshot\n       poke-around notify <message>\n       poke-around set-mode <full|limited|sandbox>\n       poke-around status"
    );
}
