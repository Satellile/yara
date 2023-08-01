use std::fs;
use std::path::PathBuf;
use std::io::{BufReader, Read};
use serde_json::Value;

use crate::data::YaraPrompt;

#[derive(PartialEq)]
enum WidgetField {
    String,
    U64,
    F64,
    Skip,
}

const EXPECTED_WIDGETS_KSAMPLER: [(&str, WidgetField); 7] = [
        ("seed", WidgetField::U64),
        ("control_after_generate", WidgetField::Skip), 
        ("steps", WidgetField::U64),
        ("cfg", WidgetField::F64),
        ("sampler_name", WidgetField::String), 
        ("scheduler", WidgetField::String),
        ("denoise", WidgetField::F64),
    ];

const EXPECTED_WIDGETS_KSAMPLER_ADVANCED: [(&str, WidgetField); 10] = [
        ("add_noise", WidgetField::String), 
        ("noise_seed", WidgetField::U64),
        ("control_after_generate", WidgetField::Skip), 
        ("steps", WidgetField::U64),
        ("cfg", WidgetField::F64),
        ("sampler_name", WidgetField::String), 
        ("scheduler", WidgetField::String),
        ("start_at_step", WidgetField::U64),
        ("end_at_step", WidgetField::U64),
        ("return_with_leftover_noise", WidgetField::String), 
    ];
// Every node has a number of input fields. 
    // Some are mandatory "inputs" (e.g. "MODEL" on a KSampler node - they must be sourced from another node)
    // Some are widgets (e.g. "seed" on a KSampler node - you can change them in ComfyUI, right on the node)
    // Some widgets may be converted to inputs.
// Widget fields in the workflow data are not named. We cannot automatically tell which field the data in "widgets" belongs to.
            // https://github.com/comfyanonymous/ComfyUI/issues/2275
// Some fields (e.g. "control_after_generate" on a KSampler node) are only a part of ComfyUI's frontend, and aren't included in API prompts. 
// 
// To reconstruct a node for !yara_unmute, we need widget field data.
// The structure of the data needs to be hard-coded.
// I don't foresee !yum being useful on anything outside of KSampler nodes, so I'm only hardcoding those.
// If ComfyUI issue #2275 is resolved, it should be easier to let any arbitrary node be unmuted with !yara_unmute.

#[derive(Debug)]
struct FlowNodeData {
    id: u64,
    muted: bool,
    custom_title: Option<String>,
    widgets: Option<Vec<Value>>,
    inputs: Option<Vec<ApiInput>>,
    kind: String,
    output_types: Option<Vec<String>>,
}
#[derive(Debug, Clone)]
struct ApiInput {
    link_id: Option<u64>,
    name: String,
    // kind: String,
}
#[derive(Debug)]
struct LinkData {
    link_id: u64,
    from_node_id: u64,
    from_node_slot: u64,
    to_node_id: u64,
    to_node_slot: u64,
    // name: String,
}
#[derive(Debug)]
struct Node {
    id: u64,
    contents: serde_json::Map<String, Value>,
}


pub fn unmute_and_regenerate(filepath: PathBuf, mut comfyui_input_directory: PathBuf) -> YaraPrompt {
    let filename = filepath.file_stem().unwrap().to_string_lossy();
    let mut yara_unmute_counter = 0;
    let mut yara_mute_counter = 0;
    let mut yara_load_here_counter = 0;

    // Read embedded data from .png file
    let file = fs::File::open(filepath.as_path()).unwrap();
    let mut reader = BufReader::new(file);

    let api_data_marker: [u8; 10] = [116, 69, 88, 116, 112, 114, 111, 109, 112, 116]; // "tEXtprompt"
    let bytes = match_header_string_and_read_data(&mut reader, api_data_marker);
    let api_data: Value = serde_json::from_slice(&bytes).unwrap();
    // println!("     API Data:\n{}\n", api_data);

    let flow_data_marker: [u8; 10] = [116, 69, 88, 116, 119, 111, 114, 107, 102, 108]; // "tEXtworkfl"
    let bytes = match_header_string_and_read_data(&mut reader, flow_data_marker);
    let mut flow_data: Value = serde_json::from_slice(&bytes).unwrap();
    // println!("    Flow Data:\n{}\n", flow_data);

    // Get node info from workflow metadata
    let mut flow_nodes: Vec<FlowNodeData> = Vec::new();
    for node in flow_data.as_object().unwrap().get("nodes").unwrap().as_array().unwrap().iter() .map(|x| {
            let obj = x.as_object().unwrap();
            FlowNodeData {
                id: obj.get("id").unwrap().as_u64().unwrap(),
                muted: match obj.get("mode").unwrap().as_u64().unwrap() {
                    2 => true,
                    _ => false,
                },
                custom_title: match obj.get("title") {
                    Some(n) => Some(n.as_str().unwrap().to_string()),
                    None => None,
                },
                widgets: match obj.get("widgets_values") {
                    Some(n) => Some(n.as_array().unwrap().to_vec()),
                    None => None,
                },
                inputs: match obj.get("inputs") {
                    Some(n) => 
                        Some(n.as_array().unwrap().iter()//.to_vec()
                            .map(|x| {
                                let obj = x.as_object().unwrap();
                                ApiInput {
                                    link_id: match obj.get("link").unwrap().as_u64() {
                                        Some(link_id) => Some(link_id),
                                        None => None,
                                    },
                                    name: obj.get("name").unwrap().as_str().unwrap().to_string(),
                                    // kind: obj.get("type").unwrap().as_str().unwrap().to_string(),
                                }
                            }).collect()
                        ),
                    
                    None => None,
                },
                kind: obj.get("type").unwrap().as_str().unwrap().to_string(),
                output_types: match obj.get("outputs") {
                    Some(output_list) => {
                        let mut output_types = Vec::new();
                        for output in output_list.as_array().unwrap() {
                            output_types.push(output.get("type").unwrap().as_str().unwrap().to_string());
                        }
                        Some(output_types)
                    },
                    None => None,
                },
            }
        }) {
        // println!("Flow Node ID: {}", node.id);
        flow_nodes.push(node);
    }

    // Get node links from workflow metadata
    let mut flow_links: Vec<LinkData> = Vec::new();
    for link in flow_data.as_object().unwrap().get("links").unwrap().as_array().unwrap() {
        let link = link.as_array().unwrap();
        let linkdata = LinkData {
            link_id: link[0].as_u64().unwrap(),
            from_node_id: link[1].as_u64().unwrap(),
            from_node_slot: link[2].as_u64().unwrap(),
            to_node_id: link[3].as_u64().unwrap(),
            to_node_slot: link[4].as_u64().unwrap(),
            // name: link[5].as_str().unwrap().to_string(),
        };
        // println!("Link: [{}, {}, {}, {}, {}, {}]", linkdata.link_id, linkdata.from_node_id, linkdata.from_node_slot, linkdata.to_node_id, linkdata.to_node_slot, linkdata.name);
        flow_links.push(linkdata);
    }


    // Begin assembling a new prompt, copying the API prompt data
    let mut new_api_nodes: Vec<Node> = Vec::new();
    let old_api_nodes = api_data.as_object().unwrap();
    for id in old_api_nodes.keys() {
        new_api_nodes.push(Node {
            id: id.parse::<u64>().unwrap(),
            contents: old_api_nodes.get(id).unwrap().as_object().unwrap().clone(),
        });
    }


    // Find the node(s) to unmute
    let mut yara_unmute_nodes: Vec<FlowNodeData> = Vec::new();
    for node in &flow_nodes {
        if node.muted {
            if let Some(ref title) = node.custom_title {
                if title.contains("!yara_unmute") | title.contains("!yum") {
                    // println!("Unmute Node ID: {} - {title}", node.id);
                    yara_unmute_nodes.push(FlowNodeData {
                        id: node.id,
                        muted: node.muted,
                        custom_title: Some(node.custom_title.as_ref().unwrap().clone()),
                        widgets: Some(node.widgets.as_ref().unwrap().clone()),
                        inputs: Some(node.inputs.as_ref().unwrap().clone()),
                        output_types: node.output_types.clone(),
                        kind: node.kind.clone(),
                    });
                }
            }
        }
    }

    // Create and add in the muted node(s)
    for new_node_flowdata in yara_unmute_nodes {

        // Begin creating the muted node using the workflow metadata
        let mut inputs = serde_json::Map::new();

        // Create input fields
        for input_widget in new_node_flowdata.inputs.unwrap() {
            let (input_node_id, input_node_slot) = get_input_source(&flow_links, &flow_nodes, input_widget.link_id.unwrap());
            inputs.insert(input_widget.name.to_string(), 
                serde_json::from_str(&format!(r#"["{input_node_id}", {input_node_slot}]"#)).unwrap()
                );
        }

        // Create non-input fields
        let expected_widgets = match new_node_flowdata.kind.as_str() {
            "KSampler" => EXPECTED_WIDGETS_KSAMPLER.iter(),
            "KSamplerAdvanced" => EXPECTED_WIDGETS_KSAMPLER_ADVANCED.iter(),
            // "SamplerCustom" => ,
            _ => { panic!("Error - !ym can only be used on certain nodes: KSampler, KSamplerAdvanced, and SamplerCustom."); /*.unwrap()*/ }
        };

        let mut widgets = new_node_flowdata.widgets.as_ref().unwrap().iter();
        for (name, need) in expected_widgets {
            let name = name.to_string();
            if !inputs.contains_key(&name) { // widget hasn't been converted to input in ComfyUI
                let w = widgets.next().unwrap();
                match need {
                    WidgetField::String => {
                        let data = w.as_str().unwrap();
                        inputs.insert(name, Value::String(data.into()));
                    }
                    WidgetField::U64 => {
                        let data = w.as_u64().unwrap();
                        inputs.insert(name, Value::Number(data.into()));
                    }
                    WidgetField::F64 => {
                        let data = w.as_f64().unwrap();
                        inputs.insert(name, Value::Number(serde_json::Number::from_f64(data).unwrap()));
                    }
                    WidgetField::Skip => (),
                }
            }
        }

        let mut node_contents = serde_json::Map::new();
        node_contents.insert("class_type".to_string(), Value::String(new_node_flowdata.kind));
        node_contents.insert("inputs".to_string(), Value::Object(inputs));

        // Create the muted node and add it into our new prompt
        new_api_nodes.push(Node {
            id: new_node_flowdata.id,
            contents: node_contents.clone(),
        });

        // Follow the now-unmuted node's output forward, connect it to the proper nodes
        for linkdata in &flow_links {
            if linkdata.from_node_id == new_node_flowdata.id {

                let (final_node_id, _) = get_output_info(linkdata, &flow_links, &flow_nodes);
                let outgoing_name = get_outgoing_name(&flow_nodes, final_node_id, linkdata.link_id);

                let new: Value = serde_json::from_str(&format!(r#"["{}", {}]"#, linkdata.from_node_id, linkdata.from_node_slot)).unwrap();
                let i = new_api_nodes.iter().position(|x| x.id == final_node_id).unwrap();
                new_api_nodes.get_mut(i).unwrap()
                    .contents
                    .get_mut("inputs").unwrap().as_object_mut().unwrap()
                    .insert(outgoing_name, new);
            }
        }

        // Un-mute the node in the flow data
        unmute_node_in_workflow_json(&mut flow_data, new_node_flowdata.id);
        yara_unmute_counter += 1;
    }
    

    // Mute (remove) any nodes marked with !ym
    for node in &flow_nodes {
        if let Some(ref title) = node.custom_title {
            if title.contains("!yara_mute") | title.contains("!ym") {
                new_api_nodes.remove(new_api_nodes.iter().position(|x| x.id == node.id).unwrap());
                // println!("Removed node {}: {title}", node.id);
                mute_node_in_workflow_json(&mut flow_data, node.id);
                yara_mute_counter += 1;
            }
        }
    }


    // Search for node(s) marked !ylh
    let mut node_ids_to_replace: Vec<u64> = Vec::new();
    for node in &flow_nodes {
        if let Some(ref title) = node.custom_title {
            if title.contains("!yara_load_here") | title.contains("!ylh") {
                if let Some(output_types) = &node.output_types {
                    if !output_types.is_empty() & output_types.contains(&"IMAGE".to_string()) {
                        // println!("Replacing node ({} - {title}) with LoadImage node", node.id);
                        node_ids_to_replace.push(node.id);
                    } else {
                        println!("Warning ({filename}) - Found a node marked with !yara_load_here ({} - {title}), but the node to replace doesn't output an image. !yara_load_here marker ignored.", node.id);
                    }
                } else {
                    println!("Warning ({filename}) - Found a node marked with !yara_load_here ({} - {title}), but the node to replace doesn't output anything. !yara_load_here marker ignored.", node.id);
                }
            }
        }
    }

    // Replace any !ylh nodes with a LoadImage node
    if !node_ids_to_replace.is_empty() {

        // Find out if an edited version of the image exists
        let extension = filepath.extension().unwrap().to_str().unwrap();
        let original_filename = filepath.file_stem().unwrap().to_str().unwrap();
        let file_name_edited = original_filename.to_owned() + &"edit." + &extension;

        let mut supposed_filepath = filepath.clone();
        supposed_filepath.pop();
        supposed_filepath.push(file_name_edited);

        let (image_path, filename) = if supposed_filepath.exists() {
            (supposed_filepath.as_path(), original_filename.to_string() + &"edit")
        } else {
            (filepath.as_path(), original_filename.to_string())
        };

        // Get image hash, get new filename, copy image to ComfyUI/inputs
        let mut file = fs::File::open(image_path).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        let mut image_hash = blake3::hash(&buf).to_hex();
        image_hash.truncate(20); // Shorter => easier on the eyes, esp. in ComfyUI's jank text fields. A collision is far from catastrophic, anyway. 

        let new_filename = "_".to_owned() + &filename + &image_hash.as_str() + &"." + &extension;
        comfyui_input_directory.push(new_filename.clone());
        fs::copy(image_path, comfyui_input_directory).unwrap();


        for node_id_to_replace in node_ids_to_replace {

            // Make a LoadImage node
            let load_image_node = Node {
                id: node_id_to_replace,
                contents: serde_json::from_str(&format!(r#"
                {{
                    "class_type": "LoadImage",
                    "inputs": {{
                        "image": "{new_filename}",
                        "upload": "image"
                    }}
                }}
                "#)).unwrap(),
            };

            // Replace the original !ylh node.
            // Since both the original and replacement nodes output an 'IMAGE' and share an ID, there should be no need to modify the links/inputs of other nodes.
            for node in &mut new_api_nodes {
                if node.id == node_id_to_replace {
                    *node = load_image_node;
                    break;
                }
            }
            replace_node_loadimage_in_workflow_json(&mut flow_data, node_id_to_replace, &new_filename);
            yara_load_here_counter += 1;
        }
    }





    // Add option to randomize seed?





    let mut json_prompt = serde_json::Map::new();
    for node in new_api_nodes {
        json_prompt.insert(node.id.to_string(), Value::Object(node.contents));
    }
    let mut prompt = serde_json::Map::new();
    prompt.insert("prompt".to_string(), Value::Object(json_prompt));

    println!("{filename} - {yara_unmute_counter} nodes unmuted, {yara_mute_counter} nodes muted, {yara_load_here_counter} nodes replaced with LoadImage node.");


    YaraPrompt::new(Value::Object(prompt), flow_data)
}




pub fn match_header_string_and_read_data<R: Read>(reader: &mut BufReader<R>, header: [u8; 10]) -> Vec<u8> {
    let mut buf = [0u8; 5];
    'search_for_marker: loop {
        reader.read_exact(&mut buf).unwrap(); // UnexpectedEof: no header found
        for i in 0..(header.len()-4) {
            if header[i..(i+5)] == buf {
                // println!("\nmatched '{}' as subset of header", String::from_utf8_lossy(&buf));
                break 'search_for_marker;
            }
        }
    }

    let mut byte = [0u8; 1];
    let mut api_data: Vec<u8> = Vec::new();
    loop {
        reader.read_exact(&mut byte).unwrap();
        if byte == [b'{'] {
            api_data.push(byte[0]);
            break;
        }
    }
    let mut opening_bracket_count = 1;
    while opening_bracket_count > 0 {
        reader.read_exact(&mut byte).unwrap();
        match byte {
            [b'{'] => opening_bracket_count += 1,
            [b'}'] => opening_bracket_count -= 1,
            _ => (),
        }
        api_data.push(byte[0]);
    }

    api_data
}
fn link_input_is_reroute(flow_nodes: &Vec<FlowNodeData>, linkdata: &LinkData) -> bool {
    if "Reroute" == flow_nodes.get(flow_nodes.iter().position(|x| x.id == linkdata.from_node_id).unwrap()).unwrap().kind {
        true
    } else {
        false
    }
}
fn get_input_source(flow_links: &Vec<LinkData>, flow_nodes: &Vec<FlowNodeData>, link_id: u64) -> (u64, u64) {
    let mut linkdata = flow_links.get(flow_links.iter().position(|x| x.link_id == link_id).unwrap()).unwrap();
    if link_input_is_reroute(&flow_nodes, linkdata) {
        loop {
            linkdata = flow_links.get(flow_links.iter().position(|x| x.to_node_id == linkdata.from_node_id).unwrap()).unwrap();
            if !link_input_is_reroute(&flow_nodes, linkdata) { break; }
        }
    }
    (linkdata.from_node_id, linkdata.from_node_slot)
}
fn link_output_is_reroute(flow_nodes: &Vec<FlowNodeData>, linkdata: &LinkData) -> bool {
    if "Reroute" == flow_nodes.get(flow_nodes.iter().position(|x| x.id == linkdata.to_node_id).unwrap()).unwrap().kind {
        true
    } else {
        false
    }
}
fn get_output_info(start_linkdata: &LinkData, flow_links: &Vec<LinkData>, flow_nodes: &Vec<FlowNodeData>) -> (u64, u64) {
    let mut linkdata = start_linkdata;
    if link_output_is_reroute(&flow_nodes, &linkdata) {
        loop {
            linkdata = flow_links.get(flow_links.iter().position(|x| x.from_node_id == linkdata.to_node_id).unwrap()).unwrap();
            if !link_output_is_reroute(&flow_nodes, &linkdata) { break; }
        }
    }
    (linkdata.to_node_id, linkdata.to_node_slot)
}
fn get_outgoing_name(flow_nodes: &Vec<FlowNodeData>, final_node_id: u64, link_id: u64) -> String {
    let final_node_inputs = flow_nodes.get(flow_nodes.iter().position(|x| x.id == final_node_id).unwrap()).unwrap().inputs.as_ref().unwrap();
    let input_link_data = final_node_inputs.get(final_node_inputs.iter().position(|x| x.link_id == Some(link_id)).unwrap()).unwrap();
    input_link_data.name.to_string()
}










fn change_node_mode_in_workflow_json(flow_data: &mut Value, node_id: u64, mode: u64) {
    flow_data.as_object_mut().unwrap()
        .get_mut("nodes").unwrap()
        .as_array_mut().unwrap()
        .iter_mut()
        .for_each(|x| {
            let id = x.as_object().unwrap().get("id").unwrap().as_u64().unwrap();
            if id == node_id {
                x.as_object_mut().unwrap().insert("mode".to_string(), Value::Number(serde_json::Number::from(mode)));
            }
        });
}
fn unmute_node_in_workflow_json(mut flow_data: &mut Value, unmute_node_id: u64) {
    change_node_mode_in_workflow_json(&mut flow_data, unmute_node_id, 0);
}
fn mute_node_in_workflow_json(mut flow_data: &mut Value, mute_node_id: u64) {
    change_node_mode_in_workflow_json(&mut flow_data, mute_node_id, 2);
}
fn replace_node_loadimage_in_workflow_json(flow_data: &mut Value, replace_node_id: u64, filename: &str) {
    fn get_original_node_data(flow_data: &Value, replace_node_id: u64) -> (f64, f64, f64, f64, u64, u64) {
        for node in flow_data.as_object().unwrap().get("nodes").unwrap().as_array().unwrap() {
            if node.get("id").unwrap().as_u64().unwrap() == replace_node_id {

                let pos = node.get("pos").unwrap().as_array().unwrap();
                let pos_x: f64 = pos[0].as_f64().unwrap();
                let pos_y: f64 = pos[1].as_f64().unwrap();

                let (size_0, size_1) = match node.get("size").unwrap().as_object() {
                    Some(size) => {
                        let size_0: f64 = size.get("0").unwrap().as_f64().unwrap();
                        let size_1: f64 = size.get("1").unwrap().as_f64().unwrap();
                        (size_0, size_1)
                    }
                    None => {
                        let size = node.get("size").unwrap().as_array().unwrap();
                        let size_0: f64 = size[0].as_f64().unwrap();
                        let size_1: f64 = size[1].as_f64().unwrap();
                        (size_0, size_1)
                    }
                };

                let order: u64 = node.get("order").unwrap().as_u64().unwrap();

                let outputs = node.get("outputs").unwrap().as_array().unwrap();
                let output_image_link_id: u64 = outputs.get(outputs.iter().position(|x| x.get("type").unwrap().as_str().unwrap() == "IMAGE").unwrap()).unwrap()
                    .get("links").unwrap().as_array().unwrap()[0].as_u64().unwrap();
                

                return (pos_x, pos_y, size_0, size_1, order, output_image_link_id);
            }
        }
        panic!("Failed to get original node data for replacement node (id {replace_node_id})");
    }

    let (pos_x, pos_y, size_0, size_1, order, output_image_link_id) = get_original_node_data(&flow_data, replace_node_id);

    let new_node_str = format!(r#"
    {{
            "id": {replace_node_id},
            "type": "LoadImage",
            "pos": [{pos_x}, {pos_y}],
            "size": {{
                "0": {size_0},
                "1": {size_1}
            }},
            "flags": {{
                "collapsed": false
            }},
            "order": {order},
            "mode": 0,
            "inputs": [],
            "outputs": [{{
                "name": "IMAGE",
                "type": "IMAGE",
                "links": [{output_image_link_id}],
                "shape": 3,
                "slot_index": 0
            }}, {{
                "name": "MASK",
                "type": "MASK",
                "links": [],
                "shape": 3,
                "slot_index": 1
            }}],
            "properties": {{
                "Node name for S&R": "LoadImage"
            }},
            "widgets_values": ["{filename}", "image"]
        }}
    "#);

    let new_node: Value = serde_json::from_str(&new_node_str).unwrap();

    let nodes = flow_data.as_object_mut().unwrap()
        .get_mut("nodes").unwrap()
        .as_array_mut().unwrap();
    for node in nodes {
        let id = node.as_object().unwrap().get("id").unwrap().as_u64().unwrap();
        if id == replace_node_id {
            // x.as_object_mut().unwrap().insert("mode".to_string(), Value::Number(serde_json::Number::from(mode)));
            *node = new_node;
            return;
        }
    }
}
