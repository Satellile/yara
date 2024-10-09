use std::io::{Read, BufReader, Write};
use std::path::{PathBuf, Path};
use std::fs;
use std::collections::HashMap;
use std::{thread, time::{Duration, Instant}};

use serde_json::{Value, Map};
use serde::{Serialize, Deserialize};

mod config;
mod image_preview;
mod civitai;
mod regen;
mod fix;
mod data;

use regen::regen_modified_workflows;
use fix::{generate_yara_prompts, fix_workflows_in_folders, fix_workflow_in_file};
use data::YaraPrompt;

const STATUS: &str = "\x1b[36mstatus\x1b[0m:// ";

use config::{
    get_appdata, 
    Config,
    create_new_config,
    WorkflowStorage,
    create_new_workflow_storage
};

#[derive(Debug)]
struct PromptInfo {
    id: i64,
    positive: String,
    models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Prompt {
    prompt: Value,
}
#[derive(PartialEq)]
enum SaveQueue {
    All,
    Pending,
}
#[derive(PartialEq)]
enum ImageGenInteractive {
    Repeat,
    Finish,
}
#[derive(Serialize, Deserialize)]
struct RemovePrompts {
    delete: Vec<String>,
}


fn main() {
    // Load the config file
    let config_file = get_config_file();
    let file = match std::fs::File::open(&config_file) {
        Ok(x) => x,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => { 
            println!("Welcome to yara. Creating config file...");
            create_new_config(); 
            println!("Config file created! Yara is ready to use.");
            print_help();
            return;
        }
        Err(e) => { panic!("{e}"); }
    };
    let reader = BufReader::new(file);
    let mut cfg: Config = match serde_json::from_reader(reader) {
        Ok(x) => x,
        Err(e) => panic!("Error while reading config file:\n{e}\nTry deleting your config file and running the program again.\n\n"),
    };

    // Load the workflow file
    let workflow_storage_file = get_appdata() + &"/yara/workflow_storage.json";
    let file = match std::fs::File::open(&workflow_storage_file) {
        Ok(x) => x,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => { 
            println!("Workflow storage file not found, creating new file...");
            create_new_workflow_storage(); 
            println!("Workflow storage file created!");
            std::fs::File::open(&workflow_storage_file).unwrap()
        }
        Err(e) => { panic!("{e}"); }
    };
    let reader = BufReader::new(file);
    let mut workflow_storage: WorkflowStorage = match serde_json::from_reader(reader) {
        Ok(x) => x,
        Err(e) => panic!("Error while reading workflow storage file:\n{e}\n\n"),
    };

    let ip_port = cfg.get_ip_port();
    
    let mut args = std::env::args().skip(1);
    if let Some(cmd) = args.next() {
        match cmd.to_lowercase().as_str() {
            "list" => {
                print_files();
            }
            "l" | "load" => {
                if let Some(arg) = args.next() {
                    load_queue(arg, &mut workflow_storage, &workflow_storage_file, cfg.comfyui_output_directory, ip_port);
                } 
                else { print_help(); }
            }
            "s" | "save" => {
                if let Some(arg) = args.next() {
                    match arg.as_str() {
                        "-wr" => {
                            if let Some(arg) = args.next() {
                                save_queue(arg, SaveQueue::All, ip_port);
                            } 
                            else { print_help(); }
                        }
                        _ => {
                            save_queue(arg, SaveQueue::Pending, ip_port);
                        }
                    }
                } 
                else { print_help(); }
            }
            "d" | "delete" => {
                if let Some(arg) = args.next() {
                    delete_saved_queue(arg);
                } 
                else { print_help(); }
            }
            "e" | "examine" => { examine_queue(ip_port); }
            "w" | "wait" => { wait_to_end(ip_port); }
            "c" | "caffeine" => { caffeine(); }
            "m" | "melatonin" => { melatonin(); }
            "cwm" => {
                caffeine();
                wait_to_end(ip_port);
                melatonin();
            }
            "p" | "preview" => {
                let _ = image_preview::notan_main(&cfg);
            }
            "i" | "image" => {
                while image_generation_info() == ImageGenInteractive::Repeat {}
            }
            "h" | "help" => {
                if let Some(arg) = args.next() {
                    if (arg.to_lowercase() == "regen") || (arg.to_lowercase() == "rg") {
                        print_help_regen();
                    } else { print_help(); }
                } 
                else { print_help(); }
            }
            "cai" => {
                civitai::download(&mut args);

            }
            "cancel" => {
                let mut ids: Vec<i64> = Vec::new();
                while let Some(mut arg) = args.next() {
                    if arg.contains('+') {
                        arg.retain(|c| c.is_digit(10));
                        let start = arg.parse::<i64>().unwrap();
                        for i in start..(start+100) {
                            ids.push(i);
                        }
                    } else if arg.contains("-") {
                        let nums: Vec<i64> = arg.split("-")
                            .map(|x| {
                                let mut x1 = x.to_string();
                                x1.retain(|c| c.is_digit(10));
                                x1.parse::<i64>().unwrap()
                            })
                            .collect();
                        for i in nums[0]..(nums[1] + 1) {
                            ids.push(i);
                        }
                    } else {
                        ids.push(arg.parse::<i64>().unwrap());
                    }
                }
                cancel_generations(ids, ip_port);
            }
            "config" => {
                open_config_dir();
            }
            "rg" | "regen" => {
                let args: Vec<String> = args.collect();
                let mut yara_prompts: Vec<YaraPrompt> = Vec::new();
                let mut failures: Vec<PathBuf> = Vec::new();
                if !args.is_empty() {
                    for path in args {
                        let path = PathBuf::from(path);
                        if path_is_png_file(path.as_path()) {
                            match regen_modified_workflows(&PathBuf::from(&path), cfg.get_input_dir(), &ip_port) {
                                Some(yara_prompt) => yara_prompts.push(yara_prompt),
                                None => failures.push(path),
                            }
                        } else { println!("Error - 'yara regen' doesn't currently support specifying folders to regenerate."); }
                    } 
                } else {
                    let Ok(entries) = fs::read_dir(cfg.get_regen_dir())
                        else { println!("Error - failed to read files from 'regen' directory.
                            \rThe regen directory is {}.
                            \rThis can be changed in the config (run 'yara config' and edit 'config.json').", cfg.get_regen_dir().display()); return; };
                    for entry in entries {
                        let path = entry.unwrap().path();
                        if path_is_png_file(path.as_path()) {
                            match regen_modified_workflows(&path, cfg.get_input_dir(), &ip_port) {
                                Some(yara_prompt) => yara_prompts.push(yara_prompt),
                                None => failures.push(path),
                            }
                        }
                    }
                }
                if failures.len() > 0 {
                    println!("\x1b[31m{} images failed regen preparations:\x1b[0m {failures:#?}", failures.len());
                }
                if yara_prompts.is_empty() {
                    println!("No images were detected with Yara regen keywords (!yara_unmute, !yara_mute, or !yara_load_here).");
                } else {
                    generate_yara_prompts(yara_prompts, &mut workflow_storage, &workflow_storage_file, cfg.comfyui_output_directory, ip_port);
                }
            }
            "f" | "fix" => {
                let args: Vec<String> = args.collect();
                let mut dirs: Vec<PathBuf> = Vec::new();
                if !args.is_empty() {
                    for arg in args {
                        let path = PathBuf::from(arg);
                        if path.is_dir() {
                            dirs.push(path);
                        } else {
                            fix_workflow_in_file(&mut workflow_storage, &workflow_storage_file, path);
                        }
                    } 
                    if !dirs.is_empty() {
                        fix_workflows_in_folders(&mut workflow_storage, &workflow_storage_file, dirs);
                    }
                } else {
                    fix_workflows_in_folders(&mut workflow_storage, &workflow_storage_file, cfg.get_workflow_recovery_dirs());
                }
            }
            _ => {
                println!("Unrecognized command.");
                
            }
        }
    } else { print_help(); }
}









fn save_queue(arg: String, cmd: SaveQueue, ip_port: String) {
    let queue_data = get_queue(&ip_port);

    let mut prompts: Vec<YaraPrompt> = Vec::new();
    let mut successes = 0;
    let mut failures = 0;

    if cmd == SaveQueue::All {
        if let Some(x) = queue_data["queue_running"].as_array() {
            for p in x {
                let prompt = p[2].as_object().unwrap().clone();
                let pnginfo = p[3].as_object().unwrap().get("extra_pnginfo");
                match pnginfo {
                    Some(wf) => {
                        let workflow = wf.as_object().unwrap().get("workflow").unwrap().clone();
                        prompts.push(YaraPrompt::new(prompt, workflow, &ip_port));
                        successes += 1;
                    }
                    None => {
                        println!("\x1b[31merror\x1b[0m:// failed to save prompt number {} (could not get workflow metadata).", p[0]);
                        failures += 1;
                    }
                }
            }
        }
    }

    let mut ordered_prompts: Vec<(i64, YaraPrompt)> = Vec::new();
    if let Some(x) = queue_data["queue_pending"].as_array() {
        for p in x {
            let prompt = p[2].as_object().unwrap().clone();
            let pnginfo = p[3].as_object().unwrap().get("extra_pnginfo");
            match pnginfo {
                Some(wf) => {
                    let workflow = wf.as_object().unwrap().get("workflow").unwrap().clone();
                    ordered_prompts.push((
                        p[0].as_i64().unwrap(), 
                        YaraPrompt::new(prompt, workflow, &ip_port)
                        ));
                    successes += 1;
                }
                None => {
                    println!("\x1b[31merror\x1b[0m:// failed to save prompt number {} (could not get workflow metadata).", p[0]);
                    failures += 1;
                }
            }
        }
    }
    ordered_prompts.sort_by(|a, b| a.0.cmp(&b.0));
    let (_, x): (Vec<i64>, Vec<YaraPrompt>) = ordered_prompts.iter().cloned().unzip();
    prompts.extend(x);

    println!("{successes} prompts saved. {failures} prompts attempted to save but failed due to no workflow metadata");

    if successes > 0 {
        let path = get_saved_queue_path(arg);
        serde_json::to_writer(&fs::File::create(path.clone()).unwrap(), &prompts).unwrap();
        println!("Saved to {}", path.display());
    } else {
        println!("Did not save any prompts");
    }
}


fn cancel_generations(prompt_numbers: Vec<i64>, ip_port: String) {
    let queue_data = get_queue(&ip_port);

    let mut ids: Vec<String> = Vec::new();
    let mut interrupt_active_gen = false;

    if let Some(x) = queue_data["queue_running"].as_array() {
        for p in x {
            if prompt_numbers.contains(&p[0].as_i64().unwrap()) {
                println!("  [\x1b[32m{}\x1b[0m] - {}", p[0].as_i64().unwrap(), p[1].to_string());
                interrupt_active_gen = true;
            }
        }
    }

    if let Some(x) = queue_data["queue_pending"].as_array() {
        for p in x {
            if prompt_numbers.contains(&p[0].as_i64().unwrap()) {
                println!("  [\x1b[32m{}\x1b[0m] - {}", p[0].as_i64().unwrap(), p[1].to_string());
                ids.push(p[1].as_str().unwrap().to_string());
            }
        }
    }



    let data = serde_json::to_string(&RemovePrompts{ delete: ids }).unwrap();
    let response = isahc::post(ip_port.to_string() + &"queue", data).unwrap();

    println!("Queue-Clearing Status: {:?}", response.status());
    assert!(response.status() == 200);

    if interrupt_active_gen {
        let response = isahc::post(ip_port + &"interrupt", "x").unwrap();
        println!("Active generation interrupted: {:?}", response.status());
    }
}





fn load_queue(arg: String, storage: &mut WorkflowStorage, workflow_file: &str, output_dir: PathBuf, ip_port: String) {
    let path = get_saved_queue_path(arg);
    let file = fs::File::open(path).unwrap();
    let reader = BufReader::new(file);
    let yara_prompts: Vec<YaraPrompt> = serde_json::from_reader(reader).unwrap();
    generate_yara_prompts(yara_prompts, storage, workflow_file, output_dir, ip_port);
}


fn delete_saved_queue(arg: String) {
    let path = get_saved_queue_path(arg);
    fs::remove_file(path).unwrap();
}





fn examine_queue(ip_port: String) {
    let queue_date = get_queue(&ip_port);

    let mut ordered_prompts: Vec<(i64, PromptInfo)> = Vec::new();

    if let Some(x) = queue_date["queue_pending"].as_array() {
        for p in x {
            let pinfo = get_prompt_info(p);
            ordered_prompts.push((p[0].as_i64().unwrap(), pinfo));
        }
    }
    ordered_prompts.sort_by(|a, b| b.0.cmp(&a.0));

    for (_, pinfo) in &ordered_prompts {
        print!("\x1b[32m{}: \x1b[0m", pinfo.id);
        for model in &pinfo.models {
            print!("\x1b[32m{model}, \x1b[0m");
        }
        println!("\n\x1b[32mPositive:\x1b[0m {}", pinfo.positive);
        println!("\n");
    }
    let mut count = ordered_prompts.len();



    if let Some(x) = queue_date["queue_running"].as_array() {
        for p in x {
            let pinfo = get_prompt_info(p);
            print!("\x1b[32mRunning {}: \x1b[0m", pinfo.id);
            for model in pinfo.models {
                print!("\x1b[32m{model}, \x1b[0m");
            }
            println!("\n\x1b[32mPositive:\x1b[0m {}", pinfo.positive);
            count += 1;
        }
    }

    println!("\n\x1b[36mTotal:\x1b[0m {count}");
}


fn get_prompt_info(p: &Value) -> PromptInfo {
    let queue_id = p[0].as_i64().unwrap();
    let nodes = p[2].as_object().unwrap();

    // println!("Queue ID: {queue_id}");

    let mut nodemap: HashMap<u64, Value> = HashMap::new();
    let mut sampler_id: u64 = 0;
    for n in nodes {
        if (n.1["class_type"] == "KSampler") | (n.1["class_type"] == "KSamplerAdvanced") | (n.1["class_type"] == "SamplerCustom") {
            sampler_id = n.0.to_string().parse::<u64>().unwrap();
        }
        nodemap.insert(n.0.to_string().parse::<u64>().unwrap(), n.1.clone());
    }

    // Get models (lora -> ... -> lora -> model)
    let mut curr_node_id: u64 = sampler_id;
    let mut models: Vec<String> = Vec::new();
    loop {
        let curr_node = nodemap.get(&curr_node_id).unwrap();
        if curr_node["class_type"] == "LoraLoader" {
            let mut name = curr_node["inputs"]["lora_name"].to_string();
            if name.contains(".safetensors") {
                name = name.replace(".safetensors", "");
            }
            name = name.replace("\"", "");
            models.push(name);
        }
        if curr_node["class_type"] == "CheckpointLoaderSimple" {
            let mut name = curr_node["inputs"]["ckpt_name"].to_string();
            if name.contains(".safetensors") {
                name = name.replace(".safetensors", "");
            }
            name = name.replace("\"", "");
            models.push(name);
        }

        if let Some(input_model_id) = curr_node["inputs"].get("model") {
            curr_node_id = input_model_id[0].as_u64().unwrap();
        } 
        else {
            break;
        }
    }
    // println!("models: {models:?}");

    // Get positive prompt
    let mut curr_node_id: u64 = sampler_id;
    let mut p_prompt = String::new();
    // TO-DO: Refactor this section
    loop {
        let curr_node = nodemap.get(&curr_node_id).unwrap();
        if curr_node["class_type"] == "PromptText" {
            p_prompt = curr_node["inputs"]["text"].to_string();
            break;
        }
        if curr_node["class_type"] == "CLIPTextEncode" {
            if let Some(input_node_id) = curr_node["inputs"].get("text") {
                curr_node_id = match input_node_id[0].as_u64() {
                    Some(x) => x,
                    None => {
                        p_prompt = curr_node["inputs"]["text"].to_string();
                        break;
                    }
                }
            } else {
                p_prompt = curr_node["inputs"]["text"].to_string();
                break;
            }
        }

        else if let Some(input_node_id) = curr_node["inputs"].get("positive") {
            curr_node_id = input_node_id[0].as_u64().unwrap();
        } 
        else if let Some(input_node_id) = curr_node["inputs"].get("conditioning") {
            curr_node_id = input_node_id[0].as_u64().unwrap();
        }
        else {
            break;
        }
    }
    p_prompt = p_prompt.replace("\"", "");
    p_prompt = p_prompt.replace("/n", " ");
    // println!("positive prompt: {p_prompt:#?}");

    PromptInfo {
        id: queue_id,
        positive: p_prompt,
        models: models,
    }

}

fn count_queue(queue_data: Value) -> usize {
    match (queue_data["queue_pending"].as_array(), queue_data["queue_running"].as_array()) {
        (Some(x), Some(y)) => { x.len() + y.len() }
        (None, Some(y)) => { y.len() }
        (Some(x), None) => { x.len() }
        (None, None) => { 0 }
    }
}


fn wait_to_end(ip_port: String) {
    let count = count_queue(get_queue(&ip_port));
    let now_total_time = Instant::now();
    let elapsed = format_seconds(now_total_time.elapsed().as_secs());
    print!("{STATUS}[{elapsed}] waiting until queue is empty... (\x1b[36m{count}\x1b[0m items remaining)");
    std::io::stdout().flush().unwrap();
    loop {
        let queue_data = get_queue(&ip_port);
        if let (Some(x), Some(y)) = (queue_data["queue_running"].as_array(), queue_data["queue_pending"].as_array()) {
            if x.is_empty() & y.is_empty() {
                println!("\nQueue is empty.");
                break;
            }
        }
        thread::sleep(Duration::from_secs(1));

        let count = count_queue(queue_data);
        let elapsed = format_seconds(now_total_time.elapsed().as_secs());
        print!("\r\r{STATUS}[{elapsed}] waiting until queue is empty... (\x1b[36m{count}\x1b[0m items remaining)");
        std::io::stdout().flush().unwrap();
    }
    println!("\nFinished waiting - took {}", format_seconds(now_total_time.elapsed().as_secs()));
}


// Return ID of the node going into this input field
fn get_input_node_id(node: &Value, field: &str) -> String {
    let x = &mut node["inputs"].as_object().unwrap()[field][0].to_string();
    x.retain(|c| ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-'].contains(&c));
    x.parse::<usize>().unwrap().to_string()
}








fn image_generation_info() -> ImageGenInteractive {
    use clipboard::{ClipboardProvider, ClipboardContext};

    // get input
    println!("Enter image filepath, or 'q' to quit:");

    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer).unwrap();
    let input = buffer.trim_end();
    if (input.to_lowercase() == "q") | (input.to_lowercase() == "quit") {
        return ImageGenInteractive::Finish;
    }
    let path: PathBuf = input.to_string().replace("\"", "").into();
    println!();



    // open image as bytes, turn to string
    let mut buffer: Vec<u8> = Vec::new();
    let mut file = match fs::File::open(path) {
        Ok(x) => x,
        Err(e) => { println!("Failed to open file: {e}");  return ImageGenInteractive::Repeat; }
    };
    file.read_to_end(&mut buffer).unwrap();
    let text = String::from_utf8_lossy(&buffer);


    // get prompt json
    let text = &text.split("tEXtprompt\0{").map(|x| x.to_string()).collect::<Vec<String>>()[1];
    let mut brackets = 1;
    let mut prompt_string = String::from("{");
    for c in text.chars() {
        match c {
            '{' => { brackets += 1; prompt_string.push(c); }
            '}' => { brackets -= 1; prompt_string.push(c); }
            _ => { prompt_string.push(c); }
        }
        if brackets == 0 { break; }
    }
    let prompt_json: Value = serde_json::from_str(&prompt_string).unwrap();
    let nodes = prompt_json.as_object().unwrap();


    // Follow conditioning from Sampler until you reach Text
    fn push_preceding_text(nodes: &Map<String, Value>, mut id: String, prompts: &mut Vec<String>, controlnets: &mut Vec<String>) {
        for _ in 0..nodes.len() {
            let target = nodes.get(&id.to_string()).unwrap();
            match target["class_type"].as_str() {
                Some("PromptText") => {
                    prompts.push("\n".to_string() + &target["inputs"].as_object().unwrap()["text"].as_str().unwrap().to_string());
                    return;
                }
                Some("CLIPTextEncode")  => {
                    match target["inputs"].as_object().unwrap()["text"].as_str() {
                        Some(x) => {
                            prompts.push("\n".to_string() + &x.to_string());
                            return;
                        }
                        None => {
                            target["inputs"].as_object().unwrap();
                            id = get_input_node_id(target, "text");
                        }
                    }
                }
                Some("ControlNetApply") => {
                    let cid = get_input_node_id(target, "control_net");
                    let controlnet_target = nodes.get(&cid.to_string()).unwrap();
                    controlnets.push(controlnet_target["inputs"].as_object().unwrap()["control_net_name"].as_str().unwrap().to_string());
                    id = get_input_node_id(target, "conditioning");
                }
                _ => {
                    id = get_input_node_id(target, "conditioning");
                }
            }
        }
        panic!("Failed to find the originating node for a sampler's input positive/negative text");
    }

    // Follow model from Sampler, accumulating loras and the model at the end
    fn push_preceding_models(nodes: &Map<String, Value>, mut id: String, loras: &mut Vec<String>, models: &mut Vec<String>) {
        for _ in 0..nodes.len() {
            let target = nodes.get(&id.to_string()).unwrap();
            match target["class_type"].as_str() {
                Some("CheckpointLoaderSimple") | Some("CheckpointLoader") => {
                    models.push(target["inputs"].as_object().unwrap()["ckpt_name"].as_str().unwrap().to_string());
                    return;
                }
                Some("LoraLoader") => {
                    loras.push(target["inputs"].as_object().unwrap()["lora_name"].as_str().unwrap().to_string());
                    id = get_input_node_id(target, "model");
                }
                _ => {
                    id = get_input_node_id(target, "model");
                }
            }
        }
        panic!("Failed to find the originating node for a sampler's input positive/negative text");
    }




    let mut pprompts: Vec<String> = Vec::new();
    let mut nprompts: Vec<String> = Vec::new();
    let mut controlnets: Vec<String> = Vec::new();
    let mut loras: Vec<String> = Vec::new();
    let mut models: Vec<String> = Vec::new();


    for node in nodes.values() {
        if let Some("KSampler") | Some("KSamplerAdvanced") = node["class_type"].as_str() {
            // Find positive prompt
            let pid = get_input_node_id(node, "positive");
            push_preceding_text(nodes, pid, &mut pprompts, &mut controlnets);

            // Find negative prompt
            let nid = get_input_node_id(node, "negative");
            push_preceding_text(nodes, nid, &mut nprompts, &mut controlnets);

            // Find loras and originating model
            let mid = get_input_node_id(node, "model");
            push_preceding_models(nodes, mid, &mut loras, &mut models,);
        }
    }

    models.dedup();
    loras.dedup();
    pprompts.dedup();
    nprompts.dedup();
    controlnets.dedup();

    for text in pprompts.iter_mut() {
        while text.contains("\n\n") {
            *text = text.replace("\n\n", "\n");
        }
    }
    for text in nprompts.iter_mut() {
        while text.contains("\n\n") {
            *text = text.replace("\n\n", "\n");
        }
    }

    for x in models { println!("\x1b[32mModel\x1b[0m: {x}"); }
    for x in loras { println!("\x1b[32mLora\x1b[0m: {x}"); }
    for x in controlnets { println!("\x1b[32mControlnet\x1b[0m: {x}"); }
    for x in pprompts { println!("\x1b[32mPositive\x1b[0m: {x}"); }
    for x in nprompts { println!("\x1b[32mNegative\x1b[0m: {x}"); }


    // copy json string to clipboard, including newlines
    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
    let x = serde_json::to_string_pretty(&prompt_json).unwrap();
    ctx.set_contents(x).unwrap();
    println!("Full generation data copied to clipboard.");
    println!("--------------\n");
    return ImageGenInteractive::Repeat;
}





fn get_queue(ip_port: &str) -> Value {
    let mut response = isahc::get(ip_port.to_string() + &"queue").unwrap();
    let mut buf = String::new();
    response.body_mut().read_to_string(&mut buf).unwrap();
    let json_data: Value = serde_json::from_str(&buf).unwrap();
    json_data
}


fn path_is_png_file(path: &Path) -> bool {
    if path.is_dir() { return false; }
    if let Some(ext) = path.extension() {
        if ext == "png" { true }
        else { false }
    } else { false }
}







fn print_help() {
    println!("Usage:
        yara                       print saved queues
        yara save [NAME]           save a queue as [specified] name
        yara save -wr [NAME]       save [specified] queue with running prompt included
        yara load [NAME]           load [specified] queue
        yara delete [NAME]         delete [specified] queue
        yara examine               list active queue, showing positive prompt and models
        yara caffeine              disable computer's sleep mode
        yara melatonin             enable computer's sleep mode
        yara wait                  wait until all prompts have finished
        yara preview               create a window previewing new files in the output directory
        yara image                 check embedded generation data of images
        yara clear [PROMPT_IDS]    delete queued generations by numerical ID
                                       e.g. 'yara clear 250 251 252'
        yara config                open directory of config file
        yara cai [URLs]            download CivitAI models/loras/etc, copying relevant info to clipboard
        yara regen [FILEPATHS]     regenerate images, modifying marked nodes (more info: run 'yara help regen')
        yara fix [PATHS]           search specified folders or files, try to embed missing workflows into images
        ");
}

fn print_help_regen() {
    println!("
        Usage: 'yara regen [FILEPATHS]'

        yara 'regen' will regenerate images, modifying any marked nodes.

        To mark a node, modify the node title in ComfyUI (right click -> Title) and add the keyword.
        Valid keywords:
            !yum  or  !yara_unmute           Unmute this node. Only valid on KSampler nodes.
            !ym   or  !yara_mute             Mute this node.
            !ylh  or  !yara_load_here        Replace this node with a LoadImage node, to load the original image.

        You can specify specific images to regenerate by adding their full filepaths as arguments (in Windows,
        you can simply drag/drop the files to the terminal window). Alternatively, with no additional arguments,
        'yara regen' will regenerate all images in the 'ComfyUI/output/regen' folder. The folder to get images from
        can be customized in the config file ('yara config' -> config.json).
        ");
}

fn print_files() {
    let mut main_dir: PathBuf = get_appdata().into();
    main_dir.push("yara");
    main_dir.push("saved_queues");

    let paths = fs::read_dir(main_dir).unwrap();
    for (count, dir_entry) in paths.enumerate() {
        println!("[{count}] {:?}", dir_entry.unwrap().path().file_stem().unwrap());
    }
}

fn get_saved_queue_path(queue_name: String) -> PathBuf {
    let mut path: PathBuf = get_appdata().into();
    path.push("yara");
    path.push("saved_queues");
    path.push(queue_name + &".json");
    path
}

pub fn get_config_file() -> String {
    get_appdata() + &"/yara/config.json"
}
pub fn format_seconds(secs: u64) -> String {
    let hours = (secs / 60) / 60;
    let minutes = (secs - (hours * 60 * 60)) / 60;
    let seconds = secs - (hours * 60 * 60) - (minutes * 60);
    format!("{hours:0>2}:{minutes:0>2}:{seconds:0>2}")
}

// windows-only

#[cfg(any(target_os = "windows"))]
fn open_config_dir() {
    println!("Opening folder with config file.");
    Command::new("explorer")
        .arg(get_appdata() + &"\\yara")
        .spawn()
        .unwrap();
}


#[cfg(any(target_os = "windows"))]
fn caffeine() {
    // C:\>powercfg /x standby-timeout-ac 0
    let _ = Command::new("powercfg")
        .arg("/x")
        .arg("standby-timeout-ac")
        .arg("0")
        .output()
        .unwrap();
    println!("Computer is caffeinated.");
}
#[cfg(any(target_os = "windows"))]

fn melatonin() {
    // C:\>powercfg /x standby-timeout-ac 30
    let _ = Command::new("powercfg")
        .arg("/x")
        .arg("standby-timeout-ac")
        .arg("30")
        .output()
        .unwrap();
    println!("Computer is sleepy.");
}



#[cfg(any(target_os = "linux"))]
fn open_config_dir() {
    println!("Your config file is located in \"{}/yara\".", get_appdata())
}
#[cfg(any(target_os = "linux"))]
fn caffeine() {
    println!("Sleep mode toggles not currently implemented for linux (i'm lazy sorry)");
}
#[cfg(any(target_os = "linux"))]
fn melatonin() {
    println!("Sleep mode toggles not currently implemented for linux (i'm lazy sorry)");
}