use std::io::Read;
use serde_json::Value;
use serde::{Serialize, Deserialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YaraPrompt {
    pub prompt: serde_json::Map<String, Value>,
    pub workflow: Value,
    pub hash: String,
}
impl YaraPrompt {
    pub fn new(nodemap: serde_json::Map<String, Value>, workflow: Value) -> YaraPrompt {
        let hash = hash_nodemap(&nodemap);
        let mut prompt = serde_json::Map::new();
        prompt.insert("prompt".to_string(), Value::Object(nodemap));
        YaraPrompt {
            prompt,
            workflow,
            hash,
        }
    }
    pub fn generate(&self) -> String {
        let prompt_string = serde_json::to_string(&self.prompt).unwrap();

        let mut response = isahc::post("http://127.0.0.1:8188/prompt", prompt_string).unwrap();
        let mut buf = String::new();
        response.body_mut().read_to_string(&mut buf).unwrap();

        let json: Value = serde_json::from_str(&buf).unwrap();
        let id = json.as_object().unwrap().get("prompt_id").unwrap().as_str().unwrap();
            // crashes if response fails (e.g. comfyui not active). maybe handle this, print some message
        println!("// Generating prompt // {id}");
        id.to_string()
    }
}





#[derive(Debug)]
struct Node {
    id: u64,
    contents: Value,
}

fn get_field(mut bytes: &mut Vec<u8>, input: &Value) {
    if let Some(field) = input.as_f64() {
        bytes.extend(field.to_be_bytes());
    } else if let Some(field) = input.as_str() {
        bytes.extend(field.bytes());
    } else if let Some(array) = input.as_array() {
        get_field(&mut bytes, &array[0]);
        get_field(&mut bytes, &array[1]);
    } else {
        println!("Warning - while hashing nodes, unexpected input");
        bytes.extend(input.to_string().bytes());
    }
}

impl Node {
    // We define a function to turn it into bytes.
    // Note that we can't just hash it directly, because it seems ComfyUI randomly
    // changes parts of the prompt (e.g. node ordering, or turning a float into an integer)
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::from(self.id.to_be_bytes());

        let class_type = self.contents.get("class_type").unwrap().as_str().unwrap();
        bytes.extend(class_type.as_bytes());

        let inputs = self.contents.get("inputs").unwrap().as_object().unwrap();
        let mut keys: Vec<String> = inputs.keys().map(|x| x.to_string()).collect(); // sort this alphabetically
        keys.sort();
        for key in keys {
            let input = inputs.get(&key).unwrap();
            get_field(&mut bytes, input);
        }

        bytes
    }
}

pub fn hash_nodemap(nodemap: &serde_json::Map<String, Value>) -> String {

    let mut nodes: Vec<Node> = Vec::new();
    for id in nodemap.keys() {
        nodes.push(Node {
            id: id.parse::<u64>().unwrap(),
            contents: nodemap.get(id).unwrap().clone(),
        });
    }
    nodes.sort_by(|a, b| a.id.partial_cmp(&b.id).unwrap()); 

    let mut hasher = blake3::Hasher::new();
    for node in nodes {
        hasher.update(&node.to_bytes());
    }
    hasher.finalize().to_hex().as_str().to_string()
}