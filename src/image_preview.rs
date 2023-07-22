use std::path::PathBuf;
use std::fs;
use std::io::{BufReader, Read};

use notan::draw::*;
use notan::prelude::*;

use crate::config::{Config, get_appdata};

#[derive(AppState)]
struct State {
    comfyui_output_directory: PathBuf,
    tex: Texture,
    image: Option<PathBuf>,
    last_image: PathBuf,
    h: i32,
    w: i32,
    ratio: f64,
    ih: i32,
    iw: i32,
    base_image: Vec<u8>,
    update_size: bool,
}

pub fn notan_main(cfg: &Config) -> Result<(), String> {
    notan::init_with(init)
        .add_plugin(notan::extra::FpsLimit::new(cfg.framerate_cap))
        .add_config(WindowConfig::new()
            .title("yara")
            .set_window_icon_data(Some(include_bytes!("assets/icon.png")))
            .size(cfg.default_window_size.0, cfg.default_window_size.1)
            .transparent(true)
            .always_on_top(cfg.always_on_top)
            .mouse_passthrough(cfg.mouse_passthrough)
            // .lazy_loop(true)  // greatly reduce CPU usage, but only updates when window is interacted with
            .resizable(true)
            .decorations(cfg.window_decorations)
        )
        .add_config(DrawConfig)
        .draw(draw)
        .update(update)
        .build()
}

fn init(app: &mut App, gfx: &mut Graphics) -> State {
    // app.window().set_position(-807, 188);

    // Load the config file
    let config_file = get_appdata() + &"\\yara\\config.json";
    let file = match std::fs::File::open(&config_file) {
        Ok(x) => x,
        Err(e) => { panic!("Error while loading config file within notan init function\n{e}"); }
    };
    let reader = BufReader::new(file);
    let cfg: Config = serde_json::from_reader(reader).unwrap();

    let default_base_image = include_bytes!("assets/default_base_image.png").to_vec();
    let base_image = match cfg.base_image {
        None => default_base_image,
        Some(path) => {
            let mut buf: Vec<u8> = Vec::new();
            let mut file = match std::fs::File::open(&path) {
                Ok(x) => x,
                Err(e) => { panic!("Error while opening base image as specified in config file\n{}\n{e}", path.display()); }
            };
            file.read_to_end(&mut buf).expect("Error while reading base image as specified in config file");
            buf
        }
    };

    app.window().set_position(cfg.default_window_position.0, cfg.default_window_position.1);

    let comfyui_output_directory = cfg.comfyui_output_directory;

    let texture = gfx
        .create_texture()
        .from_image(&base_image)
        .build()
        .unwrap();

    State {
        comfyui_output_directory,
        tex: texture,
        image: None,
        last_image: PathBuf::new(),
        h: 500,
        w: 500,
        ratio: 1.,
        ih: 500,
        iw:  500,
        base_image,
        update_size: true,
    }
}

fn draw(gfx: &mut Graphics, state: &mut State) {
    let mut draw = gfx.create_draw();
    draw.clear(Color::TRANSPARENT);

    // Check if image exists
    if let Some(image) = &state.image {
        // Check that image is not already drawn (viz. it's a new image)
        if image != &state.last_image {
            let bytes = match std::fs::read(&image) {
                Ok(x) => x,
                Err(e) => {
                    if Some(32) == e.raw_os_error() {
                        // File is already in access elsewhere. We assume it'll be freed later. 
                        // Probably a better way to handle this - panic after N attempts, maybe?
                        return;
                    }
                    else {panic!("{e}");}
                }
            };
            let texture = match gfx
                .create_texture()
                .from_image(&bytes)
                .build()
                {
                    Ok(x) => x,
                    Err(_) => {return;} // ehh maybe should handle this differently, who cares lol
                };
            state.tex = texture;
            state.last_image = image.to_path_buf();
        }
    } else { // No image; draw base image
        let texture = gfx
            .create_texture()
            .from_image(&state.base_image)
            .build()
            .unwrap();
        state.tex = texture;
        state.last_image = PathBuf::new();
    }

    draw.image(&state.tex).size(state.iw as f32, state.ih as f32);
    gfx.render(&draw);
}


fn update(app: &mut App, state: &mut State) {

    // Check for window size update
    let window = app.window();
    if state.update_size || (state.h != window.height()) || (state.w != window.width()) {
        state.h = window.height();
        state.w = window.width();

        // Resize image height/width using correct aspect ratio
        state.ih = window.height();
        state.iw = (window.height() as f64 * state.ratio).floor() as i32;
        if state.w < state.iw {
            state.iw = window.width();
            state.ih = (window.width() as f64 / state.ratio).floor() as i32;
        }

        state.update_size = false;
    }



    // Check if the ComfyUI output directory has a new image
    let dir_search = fs::read_dir(&state.comfyui_output_directory)
        .expect("Couldn't access ComfyUI directory")
        .flatten() // Remove failed
        .filter(|f| f.metadata().unwrap().is_file()) // Filter out folders
        .max_by_key(|x| x.metadata().unwrap().modified().unwrap()); // Sort by last modified

    if let Some(last) = dir_search {
        if let Ok(size) = imagesize::size(last.path()) {
            state.image = Some(last.path());
            state.ratio = size.width as f64 / size.height as f64;
            state.update_size = true;
        }
    } else {
        state.image = None;
        state.ratio = 1.;
        state.update_size = true;
    }
}

