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



pub fn regen_modified_workflows(filepath: &PathBuf, mut comfyui_input_directory: PathBuf) -> Option<YaraPrompt> {
    let filename = filepath.file_stem()?.to_string_lossy();
    let fail_str = "\x1b[31mfailure\x1b[0m:// \x1b[31m".to_string() + &filename + &".png\x1b[0m // failed to";
    let mut yara_unmute_counter = 0;
    let mut yara_mute_counter = 0;
    let mut yara_load_here_counter = 0;

    // Read embedded data from .png file
    let Ok(file) = fs::File::open(filepath.as_path())
        else { println!("{fail_str} open file"); return None; };

    let mut reader = BufReader::new(file);

    let api_data_marker: [u8; 10] = [116, 69, 88, 116, 112, 114, 111, 109, 112, 116]; // "tEXtprompt"
    let Ok(bytes) = match_header_string_and_read_data(&mut reader, api_data_marker)
        else { println!("{fail_str} read embedded API JSON data"); return None; };
    let api_data: serde_json::Map<String, Value> = match serde_json::from_slice(&bytes) {
        Ok(x) => x,
        Err(_) => match json5::from_str(&String::from_utf8(bytes).ok()?) {
            Ok(x) => x,
            Err(_) => { println!("{fail_str} deserialize embedded API JSON data"); return None; }
        }
    };

    let flow_data_marker: [u8; 10] = [116, 69, 88, 116, 119, 111, 114, 107, 102, 108]; // "tEXtworkfl"
    let Ok(bytes) = match_header_string_and_read_data(&mut reader, flow_data_marker)
        else { println!("{fail_str} read embedded workflow data"); return None; };
    let Ok(mut flow_data): Result<Value, serde_json::Error> = serde_json::from_slice(&bytes) 
        else { println!("{fail_str} deserialize embedded workflow data"); return None; };

    // Get node info from workflow metadata
    fn get_flownodes_from_metadata(flow_data: &Value) -> Option<Vec<FlowNodeData>> {
        let mut flow_nodes: Vec<FlowNodeData> = Vec::new();
        for json_node in flow_data.as_object()?.get("nodes")?.as_array()?.iter() {
            let obj = json_node.as_object()?;
            let flow_node =  FlowNodeData {
                id: obj.get("id")?.as_u64()?,
                muted: match obj.get("mode")?.as_u64()? {
                    2 => true,
                    _ => false,
                },
                custom_title: match obj.get("title") {
                    Some(n) => Some(n.as_str()?.to_string()),
                    None => None,
                },
                widgets: match obj.get("widgets_values") {
                    Some(n) => Some(n.as_array()?.to_vec()),
                    None => None,
                },
                inputs: match obj.get("inputs") {
                    Some(n) => {
                        let mut inputs: Vec<ApiInput> = Vec::new();
                        for x in n.as_array()?.iter() {
                            let obj = x.as_object()?;
                            inputs.push(ApiInput {
                                link_id: obj.get("link")?.as_u64(),
                                name: obj.get("name")?.as_str()?.to_string(),
                                // kind: obj.get("type")?.as_str()?.to_string(),
                            });
                        }
                        Some(inputs)
                    }

                    
                    None => None,
                },
                kind: obj.get("type")?.as_str()?.to_string(),
                output_types: match obj.get("outputs") {
                    Some(output_list) => {
                        let mut output_types = Vec::new();
                        for output in output_list.as_array()? {
                            let o_type = output.get("type")?;
                            if let Some(o_type_str) = o_type.as_str() {
                                output_types.push(o_type_str.to_string());
                            } else {
                                output_types.push(o_type.as_i64()?.to_string());
                            }
                        }
                        Some(output_types)
                    },
                    None => None,
                },
            };
            flow_nodes.push(flow_node);
        }
        Some(flow_nodes)
    }
    let Some(flow_nodes) = get_flownodes_from_metadata(&flow_data)
        else { println!("{fail_str} process node data from workflow metadata"); return None; };

    // Get node links from workflow metadata
    fn get_flow_links_from_metadata(flow_data: &Value) -> Option<Vec<LinkData>> {
        let mut flow_links: Vec<LinkData> = Vec::new();
        for link in flow_data.as_object()?.get("links")?.as_array()? {
            let link = link.as_array()?;
            let linkdata = LinkData {
                link_id: link[0].as_u64()?,
                from_node_id: link[1].as_u64()?,
                from_node_slot: link[2].as_u64()?,
                to_node_id: link[3].as_u64()?,
                to_node_slot: link[4].as_u64()?,
                // name: link[5].as_str()?.to_string(),
            };
            flow_links.push(linkdata);
        }
        Some(flow_links)
    }
    let Some(flow_links) = get_flow_links_from_metadata(&flow_data)
        else { println!("{fail_str} process link data from workflow metadata"); return None; };


    // Begin assembling a new prompt, copying the API prompt data
    fn get_api_nodes(api_data: &serde_json::Map<String, Value>) -> Option<Vec<Node>> {
        let mut new_api_nodes: Vec<Node> = Vec::new();
        for id in api_data.keys() {
            new_api_nodes.push(Node {
                id: id.parse::<u64>().ok()?,
                contents: api_data.get(id)?.as_object()?.clone(),
            });
        }
        Some(new_api_nodes)
    }
    let Some(mut new_api_nodes) = get_api_nodes(&api_data)
        else { println!("{fail_str} process node data from API JSON metadata"); return None; };


    // Find the node(s) to unmute
    let mut yara_unmute_nodes: Vec<FlowNodeData> = Vec::new();
    for node in &flow_nodes {
        if let Some(ref title) = node.custom_title {
            let title = title.to_lowercase();
            if title.contains("!yara_unmute") | title.contains("!yum") {
                if node.muted {
                    println!("\x1b[33mwarning\x1b[0m:// \x1b[33m{filename}.png\x1b[0m // detected !yara_unmute keyword, but node is not muted.");
                } else {
                    yara_unmute_nodes.push(FlowNodeData {
                        id: node.id,
                        muted: node.muted,
                        custom_title: Some(node.custom_title.as_ref()?.clone()),
                        widgets: Some(node.widgets.as_ref()?.clone()),
                        inputs: Some(node.inputs.as_ref()?.clone()),
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
        for input_widget in new_node_flowdata.inputs? {
            let Some((input_node_id, input_node_slot)) = get_input_source(&flow_links, &flow_nodes, input_widget.link_id?)
                else { println!("{fail_str} get an input source for !yara_unmute node"); return None; };
            inputs.insert(
                input_widget.name.to_string(), 
                serde_json::from_str(&format!(r#"["{input_node_id}", {input_node_slot}]"#)).ok()?
            );
        }

        // Create non-input fields
        let expected_widgets = match new_node_flowdata.kind.as_str() {
            "KSampler" => EXPECTED_WIDGETS_KSAMPLER.iter(),
            "KSamplerAdvanced" => EXPECTED_WIDGETS_KSAMPLER_ADVANCED.iter(),
            // "SamplerCustom" => ,
            _ => { println!("{fail_str} process widgets (!ym can only be used on KSampler, KSamplerAdvanced, and SamplerCustom nodes.)"); return None; }
        };

        let mut widgets = new_node_flowdata.widgets.as_ref()?.iter();
        for (name, need) in expected_widgets {
            let name = name.to_string();
            let w = widgets.next()?;
            match need {
                WidgetField::String => {
                    let data = w.as_str()?;
                    inputs.insert(name, Value::String(data.into()));
                }
                WidgetField::U64 => {
                    let data = w.as_u64()?;
                    inputs.insert(name, Value::Number(data.into()));
                }
                WidgetField::F64 => {
                    let data = w.as_f64()?;
                    inputs.insert(name, Value::Number(serde_json::Number::from_f64(data)?));
                }
                WidgetField::Skip => (),
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
                let Some((final_node_id, _, outgoing_name)) = get_output_info(linkdata, &flow_links, &flow_nodes)
                    else { println!("{fail_str} connect !yara_unmute node to output nodes"); return None; };

                let new: Value = serde_json::from_str(&format!(r#"["{}", {}]"#, linkdata.from_node_id, linkdata.from_node_slot)).ok()?;
                let i = new_api_nodes.iter().position(|x| x.id == final_node_id)?;
                new_api_nodes.get_mut(i)?
                    .contents
                    .get_mut("inputs")?.as_object_mut()?
                    .insert(outgoing_name, new);
            }
        }

        // Un-mute the node in the flow data
        if None == unmute_node_in_workflow_json(&mut flow_data, new_node_flowdata.id) {
            println!("{fail_str} unmute node in workflow for !yara_unmute"); return None;
        }
        yara_unmute_counter += 1;
    }
    

    // Mute (remove) any nodes marked with !ym
    for node in &flow_nodes {
        if let Some(ref title) = node.custom_title {
            let title = title.to_lowercase();
            if title.contains("!yara_mute") | title.contains("!ym") {
                new_api_nodes.remove(new_api_nodes.iter().position(|x| x.id == node.id)?);
                if None == mute_node_in_workflow_json(&mut flow_data, node.id) {
                    println!("{fail_str} mute node in workflow for !yara_mute"); return None;
                }
                yara_mute_counter += 1;
            }
        }
    }


    // Search for node(s) marked !ylh
    let mut node_ids_to_replace: Vec<u64> = Vec::new();
    for node in &flow_nodes {
        if let Some(ref title) = node.custom_title {
            let title = title.to_lowercase();
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
        let extension = filepath.extension()?.to_str()?;
        let original_filename = filepath.file_stem()?.to_str()?;
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
        let Ok(mut file) = fs::File::open(image_path)
            else { println!("{fail_str} open image file"); return None; };
        let mut buf: Vec<u8> = Vec::new();
        if let Err(e) = file.read_to_end(&mut buf) {
            println!("{fail_str} read image file: {e}");
            return None;
        }
        let mut image_hash = blake3::hash(&buf).to_hex();
        image_hash.truncate(20); // Shorter => easier on the eyes, esp. in ComfyUI's jank text fields. A collision is far from catastrophic, anyway. 

        let new_filename = "_".to_owned() + &filename + &image_hash.as_str() + &"." + &extension;
        comfyui_input_directory.push(new_filename.clone());
        if let Err(e) = fs::copy(image_path, comfyui_input_directory) {
            println!("{fail_str} copy image file to Comfyui/input: {e}");
            return None;
        }


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
                "#)).ok()?,
            };

            // Replace the original !ylh node.
            // Since both the original and replacement nodes output an 'IMAGE' and share an ID, there should be no need to modify the links/inputs of other nodes.
            for node in &mut new_api_nodes {
                if node.id == node_id_to_replace {
                    *node = load_image_node;
                    break;
                }
            }
            if None == replace_node_loadimage_in_workflow_json(&mut flow_data, node_id_to_replace, &new_filename) {
                println!("{fail_str} get original node data to replace for !yara_load_here"); return None; 
            }
            yara_load_here_counter += 1;
        }
    }


    // Add marker to randomize seed?


    let mut json_prompt = serde_json::Map::new();
    for node in new_api_nodes {
        json_prompt.insert(node.id.to_string(), Value::Object(node.contents));
    }

    if yara_unmute_counter + yara_mute_counter + yara_load_here_counter == 0 {
        println!("\x1b[31mwarning\x1b[0m:// \x1b[33m{filename}.png\x1b[0m // no nodes in this image's workflow had active keywords (!yara_unmute, !yara_mute, !yara_load_here) in their titles. Skipping.");
        return None;
    }

    let succ_str = "\x1b[32mprepped\x1b[0m:// \x1b[32m".to_string() + &filename + &".png\x1b[0m // ";
    println!("{succ_str}{yara_unmute_counter} nodes unmuted, {yara_mute_counter} nodes muted, {yara_load_here_counter} nodes replaced with LoadImage node.");
    Some(YaraPrompt::new(json_prompt, flow_data))
}




pub fn match_header_string_and_read_data<R: Read>(reader: &mut BufReader<R>, header: [u8; 10]) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = [0u8; 5];
    'search_for_marker: loop {
        reader.read_exact(&mut buf)?; // UnexpectedEof: no header found
        for i in 0..(header.len()-4) {
            if header[i..(i+5)] == buf {
                break 'search_for_marker;
            }
        }
    }

    let mut byte = [0u8; 1];
    let mut data: Vec<u8> = Vec::new();
    loop {
        reader.read_exact(&mut byte)?;
        if byte == [b'{'] {
            data.push(byte[0]);
            break;
        }
    }
    let mut opening_bracket_count = 1;
    while opening_bracket_count > 0 {
        reader.read_exact(&mut byte)?;
        match byte {
            [b'{'] => opening_bracket_count += 1,
            [b'}'] => opening_bracket_count -= 1,
            _ => (),
        }
        data.push(byte[0]);
    }

    Ok(data)
}
fn link_input_is_reroute(flow_nodes: &Vec<FlowNodeData>, linkdata: &LinkData) -> Option<bool> {
    if "Reroute" == flow_nodes.get(flow_nodes.iter().position(|x| x.id == linkdata.from_node_id)?)?.kind {
        Some(true)
    } else {
        Some(false)
    }
}
fn get_input_source(flow_links: &Vec<LinkData>, flow_nodes: &Vec<FlowNodeData>, link_id: u64) -> Option<(u64, u64)> {
    let mut linkdata = flow_links.get(flow_links.iter().position(|x| x.link_id == link_id)?)?;
    if link_input_is_reroute(&flow_nodes, linkdata)? {
        loop {
            linkdata = flow_links.get(flow_links.iter().position(|x| x.to_node_id == linkdata.from_node_id)?)?;
            if !link_input_is_reroute(&flow_nodes, linkdata)? { break; }
        }
    }
    Some((linkdata.from_node_id, linkdata.from_node_slot))
}
fn link_output_is_reroute(flow_nodes: &Vec<FlowNodeData>, linkdata: &LinkData) -> Option<bool> {
    if "Reroute" == flow_nodes.get(flow_nodes.iter().position(|x| x.id == linkdata.to_node_id)?)?.kind {
        Some(true)
    } else {
        Some(false)
    }
}
fn get_output_info(start_linkdata: &LinkData, flow_links: &Vec<LinkData>, flow_nodes: &Vec<FlowNodeData>) -> Option<(u64, u64, String)> {
    let mut linkdata = start_linkdata;
    if link_output_is_reroute(&flow_nodes, &linkdata)? {
        loop {
            linkdata = flow_links.get(flow_links.iter().position(|x| x.from_node_id == linkdata.to_node_id)?)?;
            if !link_output_is_reroute(&flow_nodes, &linkdata)? { break; }
        }
    }
    Some((linkdata.to_node_id, linkdata.to_node_slot, get_outgoing_name(&flow_nodes, linkdata.to_node_id, linkdata.link_id)?))
}
fn get_outgoing_name(flow_nodes: &Vec<FlowNodeData>, final_node_id: u64, link_id: u64) -> Option<String> {
    let final_node_inputs = flow_nodes.get(flow_nodes.iter().position(|x| x.id == final_node_id)?)?.inputs.as_ref()?;
    let input_link_data = final_node_inputs.get(final_node_inputs.iter().position(|x| x.link_id == Some(link_id))?)?;
    Some(input_link_data.name.to_string())
}










fn change_node_mode_in_workflow_json(flow_data: &mut Value, node_id: u64, mode: u64) -> Option<()> {
    for x in flow_data.as_object_mut()?.get_mut("nodes")?.as_array_mut()?.iter_mut() {
            let id = x.as_object()?.get("id")?.as_u64()?;
            if id == node_id {
                x.as_object_mut()?.insert("mode".to_string(), Value::Number(serde_json::Number::from(mode)));
            }
        }
    Some(())
}
fn unmute_node_in_workflow_json(mut flow_data: &mut Value, unmute_node_id: u64) -> Option<()> {
    change_node_mode_in_workflow_json(&mut flow_data, unmute_node_id, 0)
}
fn mute_node_in_workflow_json(mut flow_data: &mut Value, mute_node_id: u64) -> Option<()> {
    change_node_mode_in_workflow_json(&mut flow_data, mute_node_id, 2)
}
fn replace_node_loadimage_in_workflow_json(flow_data: &mut Value, replace_node_id: u64, filename: &str) -> Option<()> {
    fn get_original_node_data(flow_data: &Value, replace_node_id: u64) -> Option<(f64, f64, f64, f64, u64, u64)> {
        for node in flow_data.as_object()?.get("nodes")?.as_array()? {
            if node.get("id")?.as_u64()? == replace_node_id {

                let pos = node.get("pos")?.as_array()?;
                let pos_x: f64 = pos[0].as_f64()?;
                let pos_y: f64 = pos[1].as_f64()?;

                let (size_0, size_1) = match node.get("size")?.as_object() {
                    Some(size) => {
                        let size_0: f64 = size.get("0")?.as_f64()?;
                        let size_1: f64 = size.get("1")?.as_f64()?;
                        (size_0, size_1)
                    }
                    None => {
                        let size = node.get("size")?.as_array()?;
                        let size_0: f64 = size[0].as_f64()?;
                        let size_1: f64 = size[1].as_f64()?;
                        (size_0, size_1)
                    }
                };

                let order: u64 = node.get("order")?.as_u64()?;

                let outputs = node.get("outputs")?.as_array()?;
                for x in outputs.iter() {
                    if x.get("type")?.as_str()? == "IMAGE" {
                        let output_image_link_id: u64 = x.get("links")?.as_array()?[0].as_u64()?;
                        return Some((pos_x, pos_y, size_0, size_1, order, output_image_link_id));
                    }
                }
            }
        }
        None
    }

    let (pos_x, pos_y, size_0, size_1, order, output_image_link_id) = get_original_node_data(&flow_data, replace_node_id)?;

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

    let new_node: Value = serde_json::from_str(&new_node_str).ok()?;

    let nodes = flow_data.as_object_mut()?
        .get_mut("nodes")?
        .as_array_mut()?;
    for node in nodes {
        let id = node.as_object()?.get("id")?.as_u64()?;
        if id == replace_node_id {
            // x.as_object_mut()?.insert("mode".to_string(), Value::Number(serde_json::Number::from(mode)));
            *node = new_node;
            return Some(());
        }
    }
    Some(())
}
