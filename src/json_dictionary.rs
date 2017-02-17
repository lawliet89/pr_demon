use std::collections::BTreeMap;
use rustc_serialize::{json, Encodable};

#[derive(RustcDecodable, RustcEncodable, PartialEq, Debug, Clone)]
pub struct JsonDictionary {
    dictionary: BTreeMap<String, String>,
}

impl JsonDictionary {
    pub fn new() -> JsonDictionary {
        JsonDictionary { dictionary: BTreeMap::new() }
    }

    pub fn insert<T>(&mut self, key: &str, value: &T) -> Result<(), json::EncoderError>
        where T: Encodable
    {
        let encoded = json::encode(value)?;
        self.dictionary.insert(key.to_owned(), encoded);
        Ok(())
    }
}

#[cfg(test)]
mod json_dictionary_tests {
    use super::JsonDictionary;
    use rustc_serialize::json;

    #[derive(RustcDecodable, RustcEncodable, PartialEq, Debug)]
    struct Payload {
        pub payload: String,
    }

    #[derive(RustcDecodable, RustcEncodable, PartialEq, Debug)]
    struct OtherPayload {
        pub other: String,
    }

    static EMPTY_JSON: &'static str = "{\"dictionary\":{}}";

    #[test]
    fn a_new_dictionary_is_created() {
        let dictionary = JsonDictionary::new();
        let actual_json = json::encode(&dictionary).unwrap();
        assert_eq!(EMPTY_JSON, actual_json);
    }
}
