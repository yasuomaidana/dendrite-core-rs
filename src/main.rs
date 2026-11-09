//! `dendrite-core`: the JSON-RPC 2.0 sidecar over stdio (see `docs/ARCHITECTURE.md`).

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = dendrite_core::rpc::run(
        &args,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        std::io::stderr(),
    );
    std::process::exit(code);
}
