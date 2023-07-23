use std::io::{Read, BufReader};
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;
use std::process::Command;
use std::{thread, time::{Duration, Instant}};

use serde_json::{Value, Map};
use serde::{Serialize, Deserialize};

mod config;
mod image_preview;
mod civitai;

use config::{
    get_appdata, 
    create_new_config,
    Config,
};


#[derive(Debug)]
struct PromptInfo {
    id: i64,
    positive: String,
    models: Vec<String>,
}

#[derive(Serialize, Deserialize)]
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
    let config_file = get_appdata() + &"/yara/config.json";
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
    let cfg: Config = match serde_json::from_reader(reader) {
        Ok(x) => x,
        Err(e) => panic!("Error while reading config file:\n{e}\nTry deleting your config file and running the program again.\n\n"),
    };
    // let comfyui_ip_port = "localhost:" + &cfg.comfyui_port.to_string();

    let mut args = std::env::args().skip(1);
    if let Some(cmd) = args.next() {
        match cmd.to_lowercase().as_str() {
            "list" => {
                print_files();
            }
            "l" | "load" => {
                if let Some(arg) = args.next() {
                    load_queue(arg);
                } 
                else { print_help(); }
            }
            "s" | "save" => {
                if let Some(arg) = args.next() {
                    match arg.as_str() {
                        "-wr" => {
                            if let Some(arg) = args.next() {
                                save_queue(arg, SaveQueue::All);
                            } 
                            else { print_help(); }
                        }
                        _ => {
                            save_queue(arg, SaveQueue::Pending);
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
            "e" | "examine" => { examine_queue(); }
            "w" | "wait" => { wait_to_end(); }
            "c" | "caffeine" => { caffeine(); }
            "m" | "melatonin" => { melatonin(); }
            "cwm" => {
                caffeine();
                wait_to_end();
                melatonin();
            }
            "p" | "preview" => {
                let _ = image_preview::notan_main(&cfg);
            }
            "i" | "image" => {
                while image_generation_info() == ImageGenInteractive::Repeat {}
            }
            "h" | "help" => { print_help(); }
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
                cancel_generations(ids);
            }
            "config" => {
                open_config_dir();
            }
            _ => {
                println!("Unrecognized command.");
                
            }
        }
    } else { print_help(); }
}





fn save_queue(arg: String, cmd: SaveQueue) {
    let queue_data: String = get_queue();
    let json_data: Value = serde_json::from_str(&queue_data).unwrap();

    let mut prompts: Vec<Prompt> = Vec::new();

    if cmd == SaveQueue::All {
        if let Some(x) = json_data["queue_running"].as_array() {
            for p in x {
                prompts.push(Prompt{prompt:p[2].clone()});
            }
        }
    }

    if let Some(x) = json_data["queue_pending"].as_array() {
        for p in x {
            prompts.push(Prompt{prompt:p[2].clone()});
        }
    }

    let path = get_path(arg);
    let _ = serde_json::to_writer(&fs::File::create(path.clone()).unwrap(), &prompts);
    println!("Saved to {}", path.display());
}


fn cancel_generations(prompt_numbers: Vec<i64>) {
    let queue_data: String = get_queue();
    let json_data: Value = serde_json::from_str(&queue_data).unwrap();

    let mut ids: Vec<String> = Vec::new();
    let mut interrupt_active_gen = false;

    if let Some(x) = json_data["queue_running"].as_array() {
        for p in x {
            if prompt_numbers.contains(&p[0].as_i64().unwrap()) {
                println!("  [\x1b[32m{}\x1b[0m] - {}", p[0].as_i64().unwrap(), p[1].to_string());
                interrupt_active_gen = true;
            }
        }
    }

    if let Some(x) = json_data["queue_pending"].as_array() {
        for p in x {
            if prompt_numbers.contains(&p[0].as_i64().unwrap()) {
                println!("  [\x1b[32m{}\x1b[0m] - {}", p[0].as_i64().unwrap(), p[1].to_string());
                ids.push(remove_quotes(p[1].to_string()));
            }
        }
    }



    let data = serde_json::to_string(&RemovePrompts{ delete: ids }).unwrap();
    let response = isahc::post("http://127.0.0.1:8188/queue", data).unwrap();

    println!("Queue-Clearing Status: {:?}", response.status());
    assert!(response.status() == 200);

    if interrupt_active_gen {
        let response = isahc::post("http://127.0.0.1:8188/interrupt", "x").unwrap();
        println!("Active generation interrupted: {:?}", response.status());
    }
}





fn load_queue(arg: String) {
    let path = get_path(arg);
    let file = fs::File::open(path).unwrap();
    let reader = BufReader::new(file);
    let prompts: Vec<Value> = serde_json::from_reader(reader).unwrap();

    for p in prompts {
        let data = serde_json::to_string(&p).unwrap();

        let mut response = isahc::post("http://127.0.0.1:8188/prompt", data).unwrap();

        let mut buf = String::new();
        response.body_mut().read_to_string(&mut buf).unwrap();
        println!("Body String: {buf}");
    }

    examine_queue();
}


fn delete_saved_queue(arg: String) {
    let path = get_path(arg);
    fs::remove_file(path).unwrap();
}


fn examine_queue() {
    let queue_data = get_queue();
    let json_data: Value = serde_json::from_str(&queue_data).unwrap();

    let mut count = 0;

    if let Some(x) = json_data["queue_pending"].as_array() {
        for p in x.iter().rev() {
            let pinfo = get_prompt_info(p);
            print!("\x1b[32m{}: \x1b[0m", pinfo.id);
            for model in pinfo.models {
                print!("\x1b[32m{model}, \x1b[0m");
            }
            println!("\n\x1b[32mPositive:\x1b[0m {}", pinfo.positive);
            println!("\n");
            count += 1;
        }
    }

    if let Some(x) = json_data["queue_running"].as_array() {
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
        if n.1["class_type"] == "KSampler" {
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

            curr_node_id = remove_quotes(input_model_id[0].to_string())
                .parse::<u64>().unwrap();
        } 
        else {
            break;
        }
    }
    // println!("models: {models:?}");

    // Get positive prompt
    let mut curr_node_id: u64 = sampler_id;
    let mut p_prompt = String::new();
    loop {
        let curr_node = nodemap.get(&curr_node_id).unwrap();
        if curr_node["class_type"] == "CLIPTextEncode" {
            p_prompt = curr_node["inputs"]["text"].to_string();
            break;
        }

        if let Some(input_node_id) = curr_node["inputs"].get("positive") {
            curr_node_id = remove_quotes(input_node_id[0].to_string())
                .parse::<u64>().unwrap();
        } 
        else if let Some(input_node_id) = curr_node["inputs"].get("conditioning") {
            curr_node_id = remove_quotes(input_node_id[0].to_string())
                .parse::<u64>().unwrap();
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

fn count_queue() -> usize {
    let queue_data = get_queue();
    let json_data: Value = serde_json::from_str(&queue_data).unwrap();
    let mut count = 0;
    if let Some(x) = json_data["queue_pending"].as_array() {
        count += x.len();
    }
    if let Some(x) = json_data["queue_running"].as_array() {
        count += x.len();
    }
    count
}

fn wait_to_end() {
    let count = count_queue();
    let mut starting_count = count;
    let mut old_count = count;
    println!("Waiting until queue is empty... (\x1b[36m{count}\x1b[0m items remaining)");
    let now_total_time = Instant::now();
    let mut now_eta = Instant::now();
    let mut tracker = Instant::now();
    loop {
        let queue_data: String = get_queue();
        let json_data: Value = serde_json::from_str(&queue_data).unwrap();
        if let Some(x) = json_data["queue_running"].as_array() {
            if let Some(y) = json_data["queue_pending"].as_array() {
                if x.is_empty() & y.is_empty() {
                    println!("Queue is empty.");
                    break;
                }
            }
        }
        thread::sleep(Duration::from_secs(5));

        // Every 5 minutes, tell user the remaining number of items
        if tracker.elapsed().as_secs() > 300 {
            tracker = Instant::now();
            let count = count_queue();

            if old_count < count { // More gens were added; reset 
                println!("Detected new queues added; resetting ETA calculations");
                starting_count = count;
                now_eta = Instant::now();
                println!("Waiting until queue is empty... (\x1b[36m{count}\x1b[0m items remaining)");
            } 
            else if starting_count == count { // No queues have completed. We have no ETA
                println!("Waiting until queue is empty... (\x1b[36m{count}\x1b[0m items remaining)");
            }
            else {
                let avg_gen_time = now_eta.elapsed().as_secs() as f64 / (starting_count - count) as f64; // Average seconds  for one gen. plus/minus 5 sec
                let eta_secs = (avg_gen_time * count as f64).round() as u64;
                let eta_hours = (eta_secs / 60) / 60;
                let eta_minutes = (eta_secs - (eta_hours * 60 * 60)) / 60;
                println!("Waiting until queue is empty... (\x1b[36m{count}\x1b[0m items remaining) [eta: {eta_hours} hrs {eta_minutes} min]");
            }
            old_count = count;
        }
    }
    let secs = now_total_time.elapsed().as_secs();
    let hours = (secs / 60) / 60;
    let minutes = (secs - (hours * 60 * 60)) / 60;
    println!("Finished waiting after {hours} hrs {minutes} min");
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




    // Return ID of the node going into this input field
    fn get_input_node_id(node: &Value, field: &str) -> String {
        let x = &mut node["inputs"].as_object().unwrap()[field][0].to_string();
        x.retain(|c| ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-'].contains(&c));
        x.parse::<usize>().unwrap().to_string()
    }

    // Follow conditioning from Sampler until you reach Text
    fn push_preceding_text(nodes: &Map<String, Value>, mut id: String, prompts: &mut Vec<String>, controlnets: &mut Vec<String>) {
        for _ in 0..nodes.len() {
            let target = nodes.get(&id.to_string()).unwrap();
            match target["class_type"].as_str() {
                Some("CLIPTextEncode")  => {
                    prompts.push("\n".to_string() + &target["inputs"].as_object().unwrap()["text"].as_str().unwrap().to_string());
                    return;
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





fn get_queue() -> String {
    let mut response = isahc::get("http://127.0.0.1:8188/queue").unwrap();

    // println!("Status: {:?}", response.status());
    // println!("Body: {:?}", response.body());
    // println!("Body is empty: {:?}", response.body().is_empty());

    let mut buf = String::new();
    response.body_mut().read_to_string(&mut buf).unwrap();
    buf
}
fn remove_quotes(mut string: String) -> String {
    string.pop();
    string.remove(0);
    string
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

fn get_path(arg: String) -> PathBuf {
    let mut path: PathBuf = get_appdata().into();
    path.push("yara");
    path.push("saved_queues");
    path.push(arg + &".json");
    path
}





// windows-only

#[cfg(any(target_os = "windows"))]
fn open_config_dir() {
    println!("Opening folder with config file.");
    Command::new("explorer")
        .arg(get_appdata() + &"/yara")
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