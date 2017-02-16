use std::io::Read;
use rustc_serialize::{json, Decodable};
use reqwest;
use reqwest::{Client, Method, Error, Response, StatusCode};
use reqwest::header::{Authorization, Basic, Accept, qitem, ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel, Attr, Value};


pub struct Headers {
    pub headers: reqwest::header::Headers,
}

impl Headers {
    pub fn new() -> Headers {
        Headers { headers: reqwest::header::Headers::new() }
    }

    pub fn add_authorization_header(&mut self, credentials: &::UsernameAndPassword) -> &mut Headers {
        self.headers.set(Authorization(Basic {
            username: credentials.username().clone(),
            password: Some(credentials.password().clone()),
        }));
        self
    }

    pub fn add_accept_json_header(&mut self) -> &mut Headers {
        self.headers.set(Accept(vec![qitem(Mime(TopLevel::Application,
                                                SubLevel::Json,
                                                vec![(Attr::Charset, Value::Utf8)]))]));
        self
    }

    pub fn add_content_type_json_header(&mut self) -> &mut Headers {
        self.headers.set(ContentType(Mime(TopLevel::Application,
                                          SubLevel::Json,
                                          vec![(Attr::Charset, Value::Utf8)])));
        self
    }

    pub fn add_content_type_xml_header(&mut self) -> &mut Headers {
        self.headers.set(ContentType(Mime(TopLevel::Application,
                                          SubLevel::Xml,
                                          vec![(Attr::Charset, Value::Utf8)])));
        self
    }
}

pub fn get<T>(url: &str, headers: reqwest::header::Headers) -> Result<T, String>
    where T: Decodable
{
    request(url, reqwest::Method::Get, &None, headers, &StatusCode::Ok)
}

pub fn post<T>(url: &str, body: &str, headers: reqwest::header::Headers, status_code: &StatusCode) -> Result<T, String>
    where T: Decodable
{
    request(url,
            reqwest::Method::Post,
            &Some(body.to_owned()),
            headers,
            status_code)
}

pub fn post_raw(url: &str, body: &str, headers: reqwest::header::Headers) -> Result<Response, Error> {
    request_raw(url, reqwest::Method::Post, &Some(body.to_owned()), headers)
}

pub fn put<T>(url: &str, body: &str, headers: reqwest::header::Headers, status_code: &StatusCode) -> Result<T, String>
    where T: Decodable
{
    request(url,
            reqwest::Method::Put,
            &Some(body.to_owned()),
            headers,
            status_code)
}

fn request_raw(url: &str,
               method: Method,
               body: &Option<String>,
               headers: reqwest::header::Headers)
               -> Result<Response, Error> {

    debug!("Requesting {} with {}", url, headers);
    let client = Client::new()?;
    let request_builder = client.request(method, url);
    let request_builder = request_builder.headers(headers);

    let request_builder = match *body {
        Some(ref body_content) => request_builder.body(body_content.clone()),
        None => request_builder,
    };
    request_builder.send()
}

fn request<T>(url: &str,
              method: reqwest::Method,
              body: &Option<String>,
              headers: reqwest::header::Headers,
              status_code: &StatusCode)
              -> Result<T, String>
    where T: Decodable
{
    let mut response = request_raw(url, method, body, headers).map_err(|err| err.to_string())?;
    match response.status() {
        status if status == status_code => (),
        e @ _ => return Err(e.to_string()),
    };

    let mut json_string = String::new();
    response.read_to_string(&mut json_string).map_err(|err| err.to_string())?;

    json::decode(&json_string).map_err(|err| format!("Error parsing response: {} {}", json_string, err))
}
