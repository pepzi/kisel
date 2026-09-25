mod gb;
mod gui;
mod nes;

use std::env;
use std::path::Path;

use gui::Gui;
use minifb::Scale;

enum Machine {
    Empty,
    Gb(Box<gb::Emu>),
    Nes(Box<nes::Emu>),
}
impl Machine {
    fn view(&self) -> (usize, usize, Scale, &'static str) {
        match self {
            Machine::Gb(_) => (gb::WIDTH, gb::HEIGHT, Scale::X4, "kisel — Game Boy"),
            Machine::Nes(_) => (nes::WIDTH, nes::HEIGHT, Scale::X2, "kisel — NES"),
            Machine::Empty => (nes::WIDTH, nes::HEIGHT, Scale::X2, "kisel"),
        }
    }
}

fn open_rom(path: &str) -> Machine {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "gb" | "gbc" => Machine::Gb(Box::new(gb::Emu::load(path))),
        "nes" => Machine::Nes(Box::new(nes::Emu::load(path))),
        _ => {
            eprintln!("Okänd ROM-typ: {path}");
            Machine::Empty
        }
    }
}

fn pick_rom() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("kisel ROMs", &["gb", "gbc", "nes"])
        .add_filter("Game Boy", &["gb", "gbc"])
        .add_filter("NES", &["nes"])
        .add_filter("All files", &["*"])
        .set_title("Load ROM")
        .pick_file()
        .map(|p| p.to_string_lossy().into_owned())
}

fn main() {
    let arg = env::args().nth(1);

    if arg.as_deref() == Some("noise") {
        let mut gui = Gui::open("kisel — noise", gb::WIDTH, gb::HEIGHT, Scale::X4);
        gb::run_noise(&mut gui);
        return;
    }

    let mut machine = match arg.as_deref() {
        Some(path) => open_rom(path),
        None => Machine::Empty,
    };
    let (w, h, scale, title) = machine.view();
    let mut gui = Gui::open(title, w, h, scale);

    while gui.running() {
        let pad = gui.pad();
        let ui = gui.ui();

        if ui.exit {
            break;
        }

        if ui.load_rom
            && let Some(path) = pick_rom()
        {
            let (x, y) = gui.position();
            machine = open_rom(&path);
            let (w, h, scale, title) = machine.view();
            gui = Gui::open(title, w, h, scale);
            gui.set_position(x, y);
        }

        match &mut machine {
            Machine::Gb(emu) => {
                emu.frame(pad);
                gui.present(emu.pixels());
            }
            Machine::Nes(emu) => {
                emu.frame(pad, ui);
                gui.present(emu.pixels());
            }
            Machine::Empty => {
                gui.present_idle();
            }
        }
    }
}
