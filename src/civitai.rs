use serde_json::Value;
use regex::Regex;

const MAX_CONNECTION_ATTEMPTS: usize = 10;

use clipboard::{ClipboardProvider, ClipboardContext};

use std::env::Args;

pub fn download(args: &mut std::iter::Skip<Args>) {
    let mut should_download = true;
    let mut clipboard_string = String::new();

    while let Some(arg) = args.next() {

        if arg.as_str() == "-nd" {
            should_download = false;
        } else {

            let url_parts: Vec<&str> = arg.split('/').collect();
            let url: String = 
                url_parts[0].to_string() + &"/"        // https:
                + &url_parts[1] + &"/"     // empty space (double slash)
                // + url_parts[2] + &"/"   // civitai.com
                + &"civitai.com/api/v1/"
                + &url_parts[3] + &"/"     // models
                + &url_parts[4]            // ID
            ;

            match get_response(&url) {
                Ok(json) => {
                    let name = json["name"].to_string();
                    let description: String = remove_quotes(json["description"].to_string())
                        .replace("</p>", "\n    ")
                        .replace("<p>", "")
                        .replace("</li>", "")
                        .replace("<li>", "")
                        .replace("</ul>", "")
                        .replace("<ul>", "")
                        .replace("</u>", "")
                        .replace("<u>", "")
                        .replace("</strong>", "")
                        .replace("<strong>", "")
                        .replace("</h2>", "")
                        .replace("<h2>", "")
                        .replace("</em>", "")
                        .replace("<em>", "")
                    ;
                    let re = Regex::new(r"<a.*</a>").unwrap();
                    // there's a better way ^ but this is good enough for me :P
                    let description = re.replace_all(&description, "[link omitted]").to_string();




                    let tags: Vec<String> = json["tags"].as_array().unwrap().iter().map(|x| x.to_string()).collect();
                    let mut tag = String::from("unknown");
                    for x in tags {
                        match remove_quotes(x).as_str() {
                            "action" => { tag = "action".to_string(); }
                            "clothes" => { tag = "clothes".to_string(); }
                            _ => (),
                        }
                    }

                    let filename = json["modelVersions"].as_array().unwrap()[0]["files"].as_array().unwrap()[0]["name"].to_string();
                    let download_url = json["modelVersions"].as_array().unwrap()[0]["files"].as_array().unwrap()[0]["downloadUrl"].to_string();
                    let keystring: String = json["modelVersions"].as_array().unwrap()[0]["trainedWords"].as_array().unwrap()
                        .iter()
                        .map(|x| x.to_string())
                        .fold(String::from(" ".repeat(4)), |acc, x| acc + &remove_quotes(x) + &",\n    ");


                    if should_download {
                        open::that(download_url).unwrap();
                    }



                    let this_string = print_stuff(
                            remove_quotes(name), 
                            arg,
                            description, 
                            uppercase_first_letter(tag), 
                            keystring,
                            remove_quotes(filename),
                        );
                    clipboard_string += &this_string;
                    clipboard_string += &"\n\n\n";

                }
                Err(statuscode) => {
                    println!("\n    Failed to reach CivitAI. [{}]", statuscode);
                }
            }
        }
    }

    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
    ctx.set_contents(clipboard_string).unwrap();
}

fn print_stuff(name: String, url: String, description: String, tag: String, keystring: String, filename: String) -> String {
    let x = format!("
{tag} - {name}
{url}
{filename}
{keystring}
    {description}
");
    println!("{x}");
    x
}







fn get_response(url: &str) -> Result<Value, u16> {
    use isahc::{
        prelude::*,
    };

    let mut response = isahc::get(url).unwrap();
    let mut attempts = 0;

    while (attempts < MAX_CONNECTION_ATTEMPTS) && (!response.status().is_success()) {
        response = isahc::get(url).unwrap();
        attempts += 1;
    }
    if response.status().is_success() {
        let json: Value = response.json().unwrap();
        return Ok(json);
    } else {
        return Err(response.status().as_u16());
    }
}








fn remove_quotes(mut string: String) -> String {
    string.pop();
    string.remove(0);
    string
}

fn uppercase_first_letter(string: String) -> String {
    let mut c = string.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}