fn main() {
    // Handle CLI subcommands (e.g. `pack <dir>`) before launching the engine.
    #[cfg(all(feature = "headed", not(target_arch = "wasm32")))]
    if petrichor64::run_cli() {
        return;
    }
    petrichor64::start();
}
