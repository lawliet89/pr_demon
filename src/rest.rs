use std::io::Read;
use rustc_serialize;
use rustc_serialize::json;
use hyper;
use hyper::client::Client;
use hyper::header::{Authorization, Basic, Accept, qitem, ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel, Attr, Value};

pub struct Headers {
    pub headers: hyper::header::Headers
}

impl Headers {
    pub fn new() -> Headers {
        Headers {
            headers: hyper::header::Headers::new()
        }
    }

    pub fn add_authorization_header(&mut self, credentials: &::UsernameAndPassword)
            -> &mut Headers {
        self.headers.set(
           Authorization(
               Basic {
                   username: credentials.username().clone(),
                   password: Some(credentials.password().clone())
               }
           )
        );
        self
    }

    pub fn add_accept_json_header(&mut self) -> &mut Headers {
        self.headers.set(
            Accept(vec![
                qitem(Mime(TopLevel::Application, SubLevel::Json,
                           vec![(Attr::Charset, Value::Utf8)])),
            ])
        );
        self
    }

    pub fn add_content_type_json_header(&mut self) -> &mut Headers {
        self.headers.set(
            ContentType(Mime(TopLevel::Application, SubLevel::Json,
                             vec![(Attr::Charset, Value::Utf8)]))
        );
        self
    }

    pub fn add_content_type_xml_header(&mut self) -> &mut Headers {
        self.headers.set(
            ContentType(Mime(TopLevel::Application, SubLevel::Xml,
                             vec![(Attr::Charset, Value::Utf8)]))
        );
        self
    }
}

pub fn get<T>(url: &str, headers: &hyper::header::Headers) -> Result<T, String>
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

pub fn post<T>(url: &str, body: &str, headers: &hyper::header::Headers, status_code: &hyper::status::StatusCode)
         -> Result<T, String> where T: rustc_serialize::Decodable {
    let mut response = match post_raw(&url, &body, &headers) {
        Ok(response) => response,
        Err(err) => return Err(err.to_string())
    };

    match response.status {
        ref status if status == status_code => (),
        e @ _ => return Err(e.to_string())
    };

    let mut json_string = String::new();
    if let Err(err) = response.read_to_string(&mut json_string) {
        return Err(err.to_string())
    }

    match json::decode(&json_string) {
        Ok(decoded) => Ok(decoded),
        Err(err) => Err(format!("Error parsing response: {} {}", json_string, err))
    }
}

pub fn post_raw(url: &str, body: &str, headers: &hyper::header::Headers)
        -> Result<hyper::client::response::Response, hyper::Error> {
    let client = Client::new();
    client.post(url)
        .body(body)
        .headers(headers.to_owned()).send()
}

pub fn put<T>(url: &str, body: &str, headers: &hyper::header::Headers, status_code: &hyper::status::StatusCode)
         -> Result<T, String> where T: rustc_serialize::Decodable {
    let client = Client::new();
    let mut response = match client.put(url).body(body).headers(headers.to_owned()).send() {
        Ok(response) => response,
        Err(err) => return Err(err.to_string())
    };

    match response.status {
        ref status if status == status_code => (),
        e @ _ => return Err(e.to_string())
    };

    let mut json_string = String::new();
    if let Err(err) = response.read_to_string(&mut json_string) {
        return Err(err.to_string())
    }

    match json::decode(&json_string) {
        Ok(decoded) => Ok(decoded),
        Err(err) => Err(format!("Error parsing response: {} {}", json_string, err))
    }
}
