use std::io::Read;
use rustc_serialize;
use rustc_serialize::json;
use hyper;
use hyper::client::{Client, IntoUrl};
use hyper::header::{Headers, Authorization, Basic, Accept, qitem, ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel, Attr, Value};

pub fn add_authorization_header(headers: &mut Headers, credentials: &::UsernameAndPassword) {
    headers.set(
       Authorization(
           Basic {
               username: credentials.username().clone(),
               password: Some(credentials.password().clone())
           }
       )
    );
}

pub fn add_accept_json_header(headers: &mut Headers) {
    headers.set(
        Accept(vec![
            qitem(Mime(TopLevel::Application, SubLevel::Json,
                       vec![(Attr::Charset, Value::Utf8)])),
        ])
    );
}

pub fn add_content_type_xml_header(headers: &mut Headers) {
    headers.set(
        ContentType(Mime(TopLevel::Application, SubLevel::Xml,
                         vec![(Attr::Charset, Value::Utf8)]))
    );
}

pub fn add_content_type_json_header(headers: &mut Headers) {
    headers.set(
        ContentType(Mime(TopLevel::Application, SubLevel::Json,
                         vec![(Attr::Charset, Value::Utf8)]))
    );
}

pub fn get<T>(url: &str, headers: &Headers) -> Result<T, String>
    where T: rustc_serialize::Decodable {
    let client = Client::new();
    let mut response = match client.get(url).headers(headers.to_owned()).send() {
        Ok(response) => response,
        Err(err) => return Err(err.to_string())
    };

    match response.status {
        hyper::status::StatusCode::Ok => (),
        e @ _ => return Err(e.to_string())
    };

    let mut json_string = String::new();
    if let Err(err) = response.read_to_string(&mut json_string) {
        return Err(format!("Unable to get a list of Builds: {}", err))
    }

    match json::decode(&json_string) {
        Ok(decoded) => Ok(decoded),
        Err(err) =>  Err(format!("Error parsing response: {} {}", json_string, err))
    }
}
