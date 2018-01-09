use hyper::mime::{Attr, Mime, SubLevel, TopLevel, Value};
use reqwest;
use reqwest::{Client, Error, Method, Response, StatusCode};
use reqwest::header::{qitem, Accept, Authorization, Basic, ContentType, UserAgent};
use serde::de::DeserializeOwned;
use serde_json;

pub struct Headers {
    pub headers: reqwest::header::Headers,
}

impl Headers {
    pub fn new() -> Headers {
        let mut headers = Headers {
            headers: reqwest::header::Headers::new(),
        };
        headers.add_pr_demon_user_agent();
        headers
    }

    pub fn add_authorization_header(&mut self, credentials: &::UsernameAndPassword) -> &mut Headers {
        self.headers.set(Authorization(Basic {
            username: credentials.username().clone(),
            password: Some(credentials.password().clone()),
        }));
        self
    }

    pub fn add_accept_json_header(&mut self) -> &mut Headers {
        self.headers.set(Accept(vec![
            qitem(Mime(
                TopLevel::Application,
                SubLevel::Json,
                vec![(Attr::Charset, Value::Utf8)],
            )),
        ]));
        self
    }

    pub fn add_content_type_json_header(&mut self) -> &mut Headers {
        self.headers.set(ContentType(Mime(
            TopLevel::Application,
            SubLevel::Json,
            vec![(Attr::Charset, Value::Utf8)],
        )));
        self
    }

    pub fn add_content_type_xml_header(&mut self) -> &mut Headers {
        self.headers.set(ContentType(Mime(
            TopLevel::Application,
            SubLevel::Xml,
            vec![(Attr::Charset, Value::Utf8)],
        )));
        self
    }

    pub fn add_pr_demon_user_agent(&mut self) -> &mut Headers {
        self.headers.set(UserAgent("pr_demon/0.1.0".to_string()));
        self
    }
}

pub fn get<T>(url: &str, headers: reqwest::header::Headers) -> Result<T, String>
where
    T: DeserializeOwned,
{
    request(url, reqwest::Method::Get, &None, headers, &StatusCode::Ok)
}

pub fn post<T>(url: &str, body: &str, headers: reqwest::header::Headers, status_code: &StatusCode) -> Result<T, String>
where
    T: DeserializeOwned,
{
    request(
        url,
        reqwest::Method::Post,
        &Some(body.to_owned()),
        headers,
        status_code,
    )
}

pub fn post_raw(url: &str, body: &str, headers: reqwest::header::Headers) -> Result<Response, Error> {
    request_raw(url, reqwest::Method::Post, &Some(body.to_owned()), headers)
}

pub fn put<T>(url: &str, body: &str, headers: reqwest::header::Headers, status_code: &StatusCode) -> Result<T, String>
where
    T: DeserializeOwned,
{
    request(
        url,
        reqwest::Method::Put,
        &Some(body.to_owned()),
        headers,
        status_code,
    )
}

fn request_raw(
    url: &str,
    method: Method,
    body: &Option<String>,
    headers: reqwest::header::Headers,
) -> Result<Response, Error> {
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

fn request<T>(
    url: &str,
    method: reqwest::Method,
    body: &Option<String>,
    headers: reqwest::header::Headers,
    status_code: &StatusCode,
) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let response = request_raw(url, method, body, headers).map_err(|err| err.to_string())?;
    match response.status() {
        status if status == status_code => (),
        e => return Err(e.to_string()),
    };

    serde_json::from_reader(response).map_err(|err| format!("Error parsing response: {}", err))
}
