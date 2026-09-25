use minifb::{Key, MENU_KEY_CTRL, Menu, Scale, Window, WindowOptions};

const ID_LOAD_ROM: usize = 1;
const ID_EXIT: usize = 2;
const ID_SAVE_STATE: usize = 3;
const ID_LOAD_STATE: usize = 4;

#[derive(Clone, Copy, Default)]
pub struct Pad {
    pub a: bool,
    pub b: bool,
    pub select: bool,
    pub start: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Clone, Copy, Default)]
pub struct Ui {
    pub debug: bool,
    pub save: bool,
    pub load: bool,
    pub rewind: bool,
    pub load_rom: bool,
    pub exit: bool,
}

pub struct Gui {
    window: Window,
    width: usize,
    height: usize,
    exit: bool,
    idle: Vec<u32>,
}

impl Gui {
    pub fn open(title: &str, width: usize, height: usize, scale: Scale) -> Self {
        let mut window = Window::new(
            title,
            width,
            height,
            WindowOptions {
                scale,
                ..WindowOptions::default()
            },
        )
        .unwrap_or_else(|e| panic!("{e}"));
        window.set_target_fps(60);
        attach_menus(&mut window);
        Self {
            window,
            width,
            height,
            exit: false,
            idle: vec![0xFF101010; width * height],
        }
    }

    pub fn running(&self) -> bool {
        self.window.is_open() && !self.window.is_key_down(Key::Escape) && !self.exit
    }

    pub fn pad(&self) -> Pad {
        let w = &self.window;
        Pad {
            a: w.is_key_down(Key::X),
            b: w.is_key_down(Key::Z),
            select: w.is_key_down(Key::Tab),
            start: w.is_key_down(Key::Enter),
            up: w.is_key_down(Key::W) | w.is_key_down(Key::Up),
            down: w.is_key_down(Key::S) | w.is_key_down(Key::Down),
            left: w.is_key_down(Key::A) | w.is_key_down(Key::Left),
            right: w.is_key_down(Key::D) | w.is_key_down(Key::Right),
        }
    }

    pub fn ui(&mut self) -> Ui {
        let mut ui = Ui {
            debug: self.window.is_key_pressed(Key::P, minifb::KeyRepeat::No),
            save: self.window.is_key_down(Key::F5),
            load: self.window.is_key_down(Key::F6),
            rewind: self.window.is_key_down(Key::R),
            load_rom: false,
            exit: false,
        };

        if let Some(id) = self.window.is_menu_pressed() {
            match id {
                ID_LOAD_ROM => ui.load_rom = true,
                ID_EXIT => {
                    ui.exit = true;
                    self.exit = true;
                }
                ID_SAVE_STATE => ui.save = true,
                ID_LOAD_STATE => ui.load = true,
                _ => {}
            }
        }

        ui
    }

    pub fn present(&mut self, buffer: &[u32]) {
        self.window
            .update_with_buffer(buffer, self.width, self.height)
            .unwrap();
    }

    pub fn present_idle(&mut self) {
        let buf = self.idle.clone();
        self.present(&buf);
    }

    pub fn position(&self) -> (isize, isize) {
        self.window.get_position()
    }

    pub fn set_position(&mut self, x: isize, y: isize) {
        self.window.set_position(x, y);
    }
}

fn attach_menus(window: &mut Window) {
    let mut file = Menu::new("File").expect("File-meny");
    file.add_item("Load ROM...", ID_LOAD_ROM)
        .shortcut(Key::O, MENU_KEY_CTRL)
        .build();
    file.add_separator();
    file.add_item("Exit", ID_EXIT)
        .shortcut(Key::Q, MENU_KEY_CTRL)
        .build();

    let mut state = Menu::new("State").expect("State-meny");
    state
        .add_item("Save state", ID_SAVE_STATE)
        .shortcut(Key::F5, 0)
        .build();
    state
        .add_item("Load state", ID_LOAD_STATE)
        .shortcut(Key::F6, 0)
        .build();

    window.add_menu(&file);
    window.add_menu(&state);
}
