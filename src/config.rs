use std::fs;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use native_dialog::FileDialog;

use crate::get_config_file;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    /// How many minutes of inactivity your computer should wait before going to sleep, as set after running "yara melatonin" .
    pub melatonin_sleep_mode_timer: usize,

    /// The "output" folder in your ComfyUI directory. This is where 'yara preview' will look for newly generated images to display.
    pub comfyui_output_directory: PathBuf,

    /// The "input" folder in your ComfyUI directory.
    comfyui_input_directory: Option<PathBuf>,

    /// The "regen" folder. Running 'yara regen' will grab every image in this folder, change any nodes marked with !yum, !ym, or !ylh, and regenerate it.
    /// If no folder is specified, the default path is ComfyUI/output/regen
    regen_directory: Option<PathBuf>,

    /// After generating images through yara (such as with 'yara load'), the workflow data is not automatically embedded into the image by ComfyUI.
    /// Yara needs to actively track ComfyUI output and manually embed the workflow data. 
    /// If Yara is closed or crashes during this process, the workflow will not be added. You can try adding workflows post-generation with 'yara fix'.
    /// By default, yara only checks the ComfyUI output folder for images missing workflows. You may add additional folders to be checked here. 
    workflow_recovery_directories: Option<Vec<PathBuf>>,

    pub comfyui_port: Option<String>,
    pub comfyui_address: Option<String>,

    /// The default window position for 'yara preview'. For multiple monitors, you can include negative coordinates/numbers to move to the left.
    pub default_window_position: (i32, i32),

    /// The default window size for 'yara preview'.
    pub default_window_size: (u32, u32),

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


impl Config {
    pub fn get_input_dir(&mut self) -> PathBuf {
        match &self.comfyui_input_directory {
            Some(x) => x.to_path_buf(),
            None => {
                self.comfyui_input_directory = Some(derive_input_path_from_output_path(&self.comfyui_output_directory.clone()));
                fs::write(get_config_file(), serde_json::to_string_pretty(&self).unwrap()).unwrap();
                self.comfyui_input_directory.clone().unwrap()
            }
        }
    }
    pub fn get_regen_dir(&mut self) -> PathBuf {
        match &self.regen_directory {
            Some(x) => x.to_path_buf(),
            None => {
                self.regen_directory = Some(derive_regen_path_from_output_path(&self.comfyui_output_directory.clone()));
                fs::write(get_config_file(), serde_json::to_string_pretty(&self).unwrap()).unwrap();
                self.regen_directory.clone().unwrap()
            }
        }
    }
    pub fn get_workflow_recovery_dirs(&mut self) -> Vec<PathBuf> {
        match &self.workflow_recovery_directories {
            Some(x) => x.to_vec(),
            None => {
                self.workflow_recovery_directories = Some(Vec::from([self.comfyui_output_directory.clone()]));
                fs::write(get_config_file(), serde_json::to_string_pretty(&self).unwrap()).unwrap();
                self.workflow_recovery_directories.clone().unwrap()
            }
        }
    }
    pub fn get_ip_port(&self) -> String {
        let mut ip_port = "http://".to_string();
        match &self.comfyui_address {
            None => { ip_port += &"localhost"; }
            Some(x) => { ip_port += &x; }
        }
        ip_port += &"/";
        match &self.comfyui_port {
            None => { ip_port += &"8188"; }
            Some(x) => { ip_port += &x; }
        }
        ip_port += &"/";
        if (None != self.comfyui_address) || (None != self.comfyui_port) {
            println!("Using non-default address/port for ComfyUI: {ip_port}");
        }
        ip_port
    }
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

    let comfyui_input_directory = derive_input_path_from_output_path(&comfyui_output_directory);
    let regen_directory = derive_regen_path_from_output_path(&comfyui_output_directory);
    let workflow_recovery_directories = Vec::from([comfyui_output_directory.clone()]);

    let cfg = Config {
        melatonin_sleep_mode_timer: 30,
        comfyui_output_directory,
        comfyui_input_directory: Some(comfyui_input_directory),
        regen_directory: Some(regen_directory),
        workflow_recovery_directories: Some(workflow_recovery_directories),
        comfyui_port: None,
        comfyui_address: None,
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

fn derive_input_path_from_output_path(comfyui_output_path: &PathBuf) -> PathBuf {
    let mut input_path = comfyui_output_path.clone();
    input_path.pop();
    input_path.push("input");
    input_path 
}

fn derive_regen_path_from_output_path(comfyui_output_path: &PathBuf) -> PathBuf {
    let mut regen_path = comfyui_output_path.clone();
    regen_path.push("regen");
    regen_path 
}









#[derive(Serialize, Deserialize, Debug)]
pub struct WorkflowStorage {
    pub workflows: HashMap<String, serde_json::Value>,
}

pub fn create_new_workflow_storage() {
    let mut workflow_storage_root = get_appdata();
    workflow_storage_root += "/yara";

    let workflows = WorkflowStorage {
        workflows: HashMap::new(),
    };
    fs::write(workflow_storage_root + &"/workflow_storage.json", serde_json::to_string_pretty(&workflows).unwrap()).unwrap();
}
