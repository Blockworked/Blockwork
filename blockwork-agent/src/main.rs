//! `blockwork-agent` binary: CLI by default, `mcp` subcommand for the MCP
//! server. Subcommands are generated from `tools.rs` (one per tool, plus
//! `version` and the `agent input/channel` toggles).

use blockwork_agent::tools::tool_table;
use serde_json::{Value, json};

fn usage() -> String {
    let mut s = String::from(
        "blockwork-agent - automate Blockwork macros and the desktop\n\
         \n\
         Usage:\n  \
         blockwork-agent <command> [args] [--key value ...] [--json]\n  \
         blockwork-agent mcp\n  \
         blockwork-agent version\n\
         \n\
         Commands (from tools.rs):\n",
    );
    for t in tool_table() {
        s.push_str(&format!("  {:<28} {}\n", t.cli, t.description));
    }
    s.push_str(
        "\n\
         Global flags: --json, --daemon PATH (ignored, socket only)\n\
         Structured errors are printed as JSON on stderr (exit 1).\n",
    );
    s
}

fn parse_kv(tokens: &[String]) -> (Vec<String>, Value) {
    let mut positional = Vec::new();
    let mut map = serde_json::Map::new();
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        if let Some(key) = t.strip_prefix("--") {
            let key = key.replace('-', "_");
            if key == "json" || key == "daemon" {
                if key == "daemon" {
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            if i + 1 < tokens.len() {
                map.insert(key, infer(&tokens[i + 1]));
                i += 2;
            } else {
                map.insert(key, Value::Bool(true));
                i += 1;
            }
        } else {
            positional.push(t.clone());
            i += 1;
        }
    }
    (positional, Value::Object(map))
}

fn infer(text: &str) -> Value {
    if let Some((k, v)) = text.split_once('=') {
        return json!({k: infer(v)});
    }
    if let Ok(n) = text.parse::<i64>() {
        return json!(n);
    }
    if let Ok(f) = text.parse::<f64>() {
        return json!(f);
    }
    match text {
        "true" => json!(true),
        "false" => json!(false),
        _ => json!(text),
    }
}

async fn call_wire(wire: &str, args: Value, json_out: bool) -> Result<(), String> {
    let mut client = blockwork_agent::DaemonClient::connect()
        .await
        .map_err(|e| format!("daemon unavailable: {e}"))?;
    match client.call(wire, args).await {
        Ok(v) => {
            if json_out {
                println!("{v}");
            } else {
                println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = real_main(args);
    std::process::exit(code);
}

fn real_main(args: Vec<String>) -> i32 {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" || args[0] == "help" {
        print!("{}", usage());
        return 0;
    }
    match args[0].as_str() {
        "version" => {
            println!("blockwork-agent {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "mcp" => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            match rt.block_on(blockwork_agent::mcp::run()) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("mcp error: {e}");
                    1
                }
            }
        }
        "agent" => {
            // `agent input on|off` and `agent channel on|off`.
            let ok = args.len() == 3 && (args[2] == "on" || args[2] == "off");
            if !ok {
                eprintln!("Usage: blockwork-agent agent (input|channel) (on|off)");
                return 1;
            }
            let key = match args[1].as_str() {
                "input" => "agent_input_allowed",
                "channel" => "agent_enabled",
                _ => return 1,
            };
            let value = json!(args[2] == "on");
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            match rt.block_on(call_wire(
                "agent_set_global_settings",
                json!({key: value}),
                false,
            )) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        _ => {
            let tool = tool_table()
                .iter()
                .find(|t| t.cli == args[0].replace('_', "-"));
            let tool = match tool {
                Some(t) => t,
                None => {
                    eprintln!("unknown command '{}'\n\n{}", args[0], usage());
                    return 1;
                }
            };
            let rest = args[1..].to_vec();
            let json_out = rest.iter().any(|t| t == "--json");
            let (positional, mut kv) = parse_kv(&rest);
            // First positional maps to `id`, second to `path`/`name` where relevant.
            let obj = kv.as_object_mut().unwrap();
            if !positional.is_empty() {
                if !obj.contains_key("id") {
                    obj.insert("id".into(), json!(positional[0]));
                }
                if positional.len() > 1 && !obj.contains_key("path") {
                    obj.insert("path".into(), json!(positional[1]));
                }
            }
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            match rt.block_on(call_wire(tool.wire, kv, json_out)) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
    }
}
