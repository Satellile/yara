use std::fs;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

use native_dialog::FileDialog;


#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    /// How many minutes of inactivity your computer should wait before going to sleep, as set after running "yara melatonin" .
    pub melatonin_sleep_mode_timer: usize,


    /// The "output" folder in your ComfyUI directory. This is where 'yara preview' will look for newly generated images to display.
    pub comfyui_output_directory: PathBuf,

    // pub comfyui_port: usize,

    /// The default window position for 'yara preview'. For multiple monitors, you can include negative coordinates/numbers to move to the left.
    pub default_window_position: (i32, i32),

    /// The default window size for 'yara preview'.
    pub default_window_size: (i32, i32),

    /// The filepath to an image file that you want to replace the default base image. The base image is displayed on 'yara preview' when no images are detected in the ComfyUI output folder.
    /// You can direct it to a fully transparent image if you want 'yara preview' to be invisible when the ComfyUI output folder is empty.
    pub base_image: Option<PathBuf>,

    /// Set to true if you want window decorations (i.e. the bar at the top of a window).
    pub window_decorations: bool,

    /// Set to true if you don't want the 'yara preview' window to detect mouse clicks (they will pass through to the window below).
    pub mouse_passthrough: bool,

    /// Set to true if you want the 'yara preview' window to always be visible, even if you are focused on another window.
    pub always_on_top: bool,

    /// The framerate cap. A higher cap increases CPU usage. Framerate should only be relevant when you are moving or resizing the window.
    /// I keep this low (default is 6) to minimize CPU usage, since I'm rarely moving or resizing the window.
    pub framerate_cap: u8,

}




#[cfg(any(target_os = "windows"))]
pub fn get_appdata() -> String {
    std::env::var("LOCALAPPDATA").expect("No APPDATA directory")
}

#[cfg(any(target_os = "linux"))]
pub fn get_appdata() -> String {
    std::env::var("HOME").expect("No HOME directory")
}

pub fn create_new_config() {
    let mut config_root = get_appdata();
    config_root += "/yara";

    println!("Please select the \"output\" folder in your ComfyUI directory:");
    let comfyui_output_directory: PathBuf = FileDialog::new().show_open_single_dir().unwrap().unwrap();

    let cfg = Config {
        melatonin_sleep_mode_timer: 30,
        comfyui_output_directory,
        default_window_position: (0, 0),
        default_window_size: (750, 750),
        base_image: None,
        window_decorations: false,
        mouse_passthrough: true,
        always_on_top: true,
        framerate_cap: 6,
    };
    match fs::create_dir(config_root.clone()) {
        Ok(_) => println!("    Created 'yara' directory in {}", config_root),
        Err(e) => println!("    Didn't create 'yara' directory: {e}"),
    };
    match fs::create_dir(config_root.clone() + "/saved_queues") {
        Ok(_) => println!("    Created 'saved_queues' directory in {}/saved_queues", config_root),
        Err(e) => println!("    Didn't create 'saved_queues' directory: {e}"),
    };
    fs::write(config_root + &"/config.json", serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
}