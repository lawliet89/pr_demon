use std::collections::BTreeMap;
use rustc_serialize::{json, Decodable, Encodable};

#[derive(RustcDecodable, RustcEncodable, PartialEq, Debug, Clone)]
pub struct JsonDictionary {
    dictionary: BTreeMap<String, String>,
}

impl JsonDictionary {
    pub fn new() -> JsonDictionary {
        JsonDictionary { dictionary: BTreeMap::new() }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.dictionary.clear();
    }

    pub fn get<T>(&self, key: &str) -> Option<Result<T, json::DecoderError>>
        where T: Decodable
    {
        let json = self.dictionary.get(key);
        match json {
            None => None,
            Some(ref json) => Some(json::decode(json)),
        }
    }

    #[allow(dead_code)]
    pub fn contains_key(&self, key: &str) -> bool {
        self.dictionary.contains_key(key)
    }

    #[allow(dead_code)]
    pub fn insert<T>(&mut self, key: &str, value: &T) -> Result<(), json::EncoderError>
        where T: Encodable
    {
        match json::encode(value) {
            Ok(encoded) => {
                self.dictionary.insert(key.to_owned(), encoded);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, key: &str) -> bool {
        match self.dictionary.remove(key) {
            Some(_) => true,
            None => false,
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        return self.dictionary.len();
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        return self.dictionary.is_empty();
    }
}

#[cfg(test)]
mod json_dictionary_tests {
    use super::JsonDictionary;
    use rustc_serialize::{json, Decodable};

    #[derive(RustcDecodable, RustcEncodable, PartialEq, Debug)]
    struct Payload {
        pub payload: String,
    }

    #[derive(RustcDecodable, RustcEncodable, PartialEq, Debug)]
    struct OtherPayload {
        pub other: String,
    }

    static EMPTY_JSON: &'static str = "{\"dictionary\":{}}";

    fn make_payload() -> Payload {
        Payload { payload: "foobar".to_owned() }
    }

    fn make_dictionary() -> JsonDictionary {
        let mut dictionary = JsonDictionary::new();
        dictionary.insert("payload", &make_payload())
            .expect("Payload should be serializable");
        dictionary
    }

    fn unwrap_from_json_dictionary<T>(dictionary: &JsonDictionary, key: &str) -> T
        where T: Decodable
    {
        match dictionary.get::<T>(key) {
            Some(Ok(result)) => result,
            _ => panic!("Unable to unwrap object"),
        }
    }

    #[test]
    fn a_new_dictionary_is_created() {
        let dictionary = JsonDictionary::new();
        let actual_json = json::encode(&dictionary).unwrap();
        assert_eq!(EMPTY_JSON, actual_json);
    }

    #[test]
    fn clears_removes_all_items() {
        let mut dictionary = make_dictionary();
        dictionary.clear();
        let actual_json = json::encode(&dictionary).unwrap();
        assert_eq!(EMPTY_JSON, actual_json);
    }

    #[test]
    fn gets_returns_a_deserialized_object() {
        let dictionary = make_dictionary();
        let object: Payload = unwrap_from_json_dictionary(&dictionary, &"payload");
        assert_eq!(make_payload(), object);
    }

    #[test]
    fn gets_returns_none_for_non_existing_keys() {
        let dictionary = make_dictionary();
        let object = dictionary.get::<Payload>("foobar");
        assert_eq!(None, object);
    }

    #[test]
    fn gets_returns_err_on_failed_deserialization() {
        let dictionary = make_dictionary();
        let object = dictionary.get::<OtherPayload>("payload").unwrap();
        assert_eq!(object.is_err(), true);
    }

    #[test]
    fn contains_keys_return_correctly() {
        let dictionary = make_dictionary();
        assert_eq!(dictionary.contains_key("payload"), true);
        assert_eq!(dictionary.contains_key("foobar"), false);
    }

    #[test]
    fn removes_removes_elements() {
        let mut dictionary = make_dictionary();
        assert_eq!(dictionary.remove("payload"), true);
        assert_eq!(dictionary.remove("foobar"), false);
        let actual_json = json::encode(&dictionary).unwrap();
        assert_eq!(EMPTY_JSON, actual_json);
    }

    #[test]
    fn empty_and_len_return_correct_values() {
        let mut dictionary = make_dictionary();
        assert_eq!(dictionary.len(), 1);
        assert_eq!(dictionary.is_empty(), false);
        assert_eq!(dictionary.remove("payload"), true);
        assert_eq!(dictionary.len(), 0);
        assert_eq!(dictionary.is_empty(), true);
    }
}
