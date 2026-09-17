mod gb;
// mod nes;

use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    let rom = args.get(1).map(|s| s.as_str()).unwrap_or("tetris.gb");

    if rom == "noise" {
        gb::run_graphic_noise();
        return;
    }

    let ext = Path::new(rom)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "gb" | "gbc" => gb::run(rom),
        "nes" => {
            eprintln!("NES är inte inkopplad än: {rom}");
            std::process::exit(1);
        }
        _ => {
            eprintln!("Okänd ROM-typ ({ext}): {rom}");
            eprintln!("Använd: emul8 <fil.gb|fil.nes>");
        }
    }
}
