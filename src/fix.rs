use std::fs;
use std::path::{Path, PathBuf};
use std::io::{BufReader, Read, Write};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

use crate::{WorkflowStorage, count_queue, get_queue};
use crate::data::{YaraPrompt, hash_nodemap};
use crate::{STATUS, format_seconds};

const API_DATA_MARKER: [u8; 10] = [116, 69, 88, 116, 112, 114, 111, 109, 112, 116]; // "tEXtprompt"


fn save_hash_and_workflow(x: &YaraPrompt, workflow_file: &str, storage: &mut WorkflowStorage) {
    storage.workflows.insert(x.hash.clone(), x.workflow.clone());
    fs::write(workflow_file, serde_json::to_string_pretty(&storage).unwrap()).unwrap();
}

fn remove_workflow_from_storage(hash: &str, workflow_file: &str, storage: &mut WorkflowStorage) {
    storage.workflows.remove(hash);
    fs::write(workflow_file, serde_json::to_string_pretty(&storage).unwrap()).unwrap();
}

pub fn fix_workflows_in_folders(mut storage: &mut WorkflowStorage, workflow_file: &str, dirs: Vec<PathBuf>) {
    for dir in dirs {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_dir() {
                if let Some(extension) = path.extension() {
                    if extension == "png" {
                        if let Ok(hash) = get_api_hash_from_image_file(path.as_path()) {
                            if let Some(workflow) = storage.workflows.get(&hash) {
                                inject_workflow_into_image(&path, &workflow);
                                remove_workflow_from_storage(&hash, workflow_file, &mut storage);
                                println!("       Workflow has been embedded into image {:?}", path.file_name().unwrap());
                            }
                        } else { println!("Failed to get api info and hash from file: {:?}", path.file_name().unwrap()); }
                    }
                }
            }
        }
    }
}


pub fn fix_workflow_in_file(mut storage: &mut WorkflowStorage, workflow_file: &str, path: PathBuf) {
    if !path.is_dir() {
        if let Some(extension) = path.extension() {
            if extension == "png" {
                let hash = get_api_hash_from_image_file(path.as_path()).unwrap();
                if let Some(workflow) = storage.workflows.get(&hash) {
                    inject_workflow_into_image(&path, &workflow);
                    remove_workflow_from_storage(&hash, workflow_file, &mut storage);
                    println!("       Workflow has been embedded into image {:?}", path.file_name().unwrap());
                }
            }
        }
    }
}




#[derive(PartialEq)]
enum PIDStatus {
    Existing,
    Queued,
    Finished,
}

pub fn generate_yara_prompts(
    yara_prompts: Vec<YaraPrompt>,
    mut storage: &mut WorkflowStorage,
    workflow_file: &str,
    comfyui_output_directory: PathBuf,
) {
    // get and store history
    let mut stored_history = get_history();
    let mut prompt_ids: HashMap<String, PIDStatus> = HashMap::new();
    for id in stored_history.as_object().unwrap().keys() {
        prompt_ids.insert(id.to_string(), PIDStatus::Existing);
    }

    for yara_prompt in &yara_prompts {
        save_hash_and_workflow(yara_prompt, &workflow_file, &mut storage);
    }

    println!("sending prompts to ComfyUI for generation...");
    for yara_prompt in &yara_prompts {
        let id = yara_prompt.generate();
        prompt_ids.insert(id, PIDStatus::Queued);
    }

    // begin watching history to see when a prompt has finished
    println!("Prompts have been sent to ComfyUI.\n");
    println!("Yara will now wait for images to generate, before manually embedding workflow data into them.");
    let mut workflows_embedded = 0;
    let timer = Instant::now();
    loop {

        // Exit if all prompts are either Finished (or already existed).
        let finished = prompt_ids.iter().fold(true, |acc, x| if x.1 == &PIDStatus::Queued { false } else { acc });
        if finished { break; }
        let count = count_queue(get_queue());
        print!("\r{STATUS}waiting... [ {} ] ({count} prompts in queue)               ", format_seconds(timer.elapsed().as_secs()));
        std::io::stdout().flush().unwrap();

        std::thread::sleep(std::time::Duration::from_secs(3));

        let new_history = get_history();

        if stored_history != new_history {
            stored_history = new_history.clone();

            for gpid in new_history.as_object().unwrap().keys() {
                if let Some(status) = prompt_ids.get_mut(gpid) {
                    if status == &PIDStatus::Queued { // one of our images, finished generating.

                        // Find out the file path
                        let outputs = new_history.as_object().unwrap().get(gpid).unwrap().get("outputs").unwrap().as_object().unwrap();
                        for k in outputs.keys() {
                            let info = outputs.get(k).unwrap().as_object().unwrap().get("images").unwrap().as_array().unwrap()[0].as_object().unwrap();
                            if info.get("type").unwrap() == "output" {
                                let filename = info.get("filename").unwrap().as_str().unwrap();
                                let subfolder = info.get("subfolder").unwrap().as_str().unwrap();
                                println!("{STATUS}generated image needs a workflow");
                                let mut path = comfyui_output_directory.clone();
                                if subfolder != "" {
                                    path.push(subfolder);
                                }
                                path.push(filename);

                                // Hash the API data, inject workflow if it matches
                                let hash = get_api_hash_from_image_file(path.as_path()).unwrap();
                                if let Some(workflow) = storage.workflows.get(&hash) {
                                    inject_workflow_into_image(&path, &workflow);
                                    remove_workflow_from_storage(&hash, workflow_file, &mut storage);
                                    println!("{STATUS}embedded workflow into {subfolder}/{filename}");
                                    workflows_embedded += 1;
                                } else {
                                    println!("//\x1b[31m WARNING\x1b[0m // Hash doesn't match (unexpected) // hash {hash}, file {filename}");
                                }
                                break;
                            }
                        }
                        *status = PIDStatus::Finished;
                    }
                }
            }
        }
    }
    println!("{STATUS}\x1b[32mfinished\x1b[0m //. embedded workflows into {workflows_embedded} of {} prompts generated", yara_prompts.len());
}


fn get_api_hash_from_image_file(path: &Path) -> Result<String, std::io::Error> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let bytes = crate::regen::match_header_string_and_read_data(&mut reader, API_DATA_MARKER)?;
    let x: serde_json::Map<String, Value> = serde_json::from_slice(&bytes)?;
    let hash = hash_nodemap(&x);
    Ok(hash)
}
fn get_history() -> Value {
    let mut response = isahc::get("http://127.0.0.1:8188/history").unwrap();
    let mut buf = String::new();
    response.body_mut().read_to_string(&mut buf).unwrap();
    serde_json::from_str(&buf).unwrap()
}
fn inject_workflow_into_image(image_path: &PathBuf, workflow: &Value) {
    let file = fs::File::open(image_path).unwrap();
    let mut reader = BufReader::new(file);

    let mut bytes: Vec<u8> = Vec::new();

    // look for textprompt header chars
    let mut buf = [0u8; 5];
    'search_for_marker: loop {
        reader.read_exact(&mut buf).unwrap(); // UnexpectedEof: no header found
        bytes.extend(buf);
        for i in 0..(API_DATA_MARKER.len()-4) {
            if API_DATA_MARKER[i..(i+5)] == buf {
                break 'search_for_marker;
            }
        }
    }

    // read textprompt data
    let mut byte = [0u8; 1];
    loop {
        reader.read_exact(&mut byte).unwrap();
        bytes.extend(byte);
        if byte == [b'{'] {
            break;
        }
    }
    let mut opening_bracket_count = 1;
    while opening_bracket_count > 0 {
        reader.read_exact(&mut byte).unwrap();
        bytes.extend(byte);
        match byte {
            [b'{'] => opening_bracket_count += 1,
            [b'}'] => opening_bracket_count -= 1,
            _ => (),
        }
    }

    // read textprompt CRC bytes
    for _ in 0..4 {
        reader.read_exact(&mut byte).unwrap();
        bytes.extend(byte);
    }


    // add a new tEXt metadata chunk, for the workflow data
    let png_chunk_name: [u8; 4] = [116, 69, 88, 116]; // tEXt
    let keyword: [u8; 8]  = [119, 111, 114, 107, 102, 108, 111, 119]; // "tEXtworkflow"
    let null_separator: [u8; 1] = [0x00];
    let text_data: Vec<u8> = serde_json::to_vec(&workflow).unwrap();

    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&png_chunk_name);
    hasher.update(&keyword);
    hasher.update(&null_separator);
    hasher.update(&text_data);
    let checksum: u32 = hasher.finalize();
    let length: u32 = (8 + 1 + text_data.len() ).try_into().unwrap();

    bytes.extend(length.to_be_bytes());
    bytes.extend(png_chunk_name);
    bytes.extend(keyword);
    bytes.extend(null_separator);
    bytes.extend(text_data);
    bytes.extend(checksum.to_be_bytes());

    // Read the rest of the file
    let mut buf: Vec<u8> = Vec::new();
    reader.read_to_end(&mut buf).unwrap();
    bytes.extend(buf);

    // Write the file, overwriting previous image
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(image_path)
        .unwrap();
    file.write_all(&bytes).unwrap();
}