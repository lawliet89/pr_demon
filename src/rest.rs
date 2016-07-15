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
