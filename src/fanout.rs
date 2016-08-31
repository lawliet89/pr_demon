use std::collections::BTreeMap;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::spawn;
use std::marker::Send;
use rustc_serialize::{json, Encodable, Decodable};

#[derive(RustcDecodable, RustcEncodable, PartialEq, Debug, Clone)]
pub enum OpCode {
    OpenPullRequest,
    BuildFound,
    BuildNotFound,
    BuildScheduled,
    BuildFinished { success: bool },
    BuildRunning,
    BuildQueued,
    Custom { payload: String }
}

#[derive(RustcDecodable, RustcEncodable, PartialEq, Debug, Clone)]
pub struct Message {
    pub opcode: OpCode,
    pub payload: String
}

impl Message {
    pub fn new<T>(opcode: OpCode, payload: &T) -> Message where T : Encodable {
        let encoded = json::encode(payload).unwrap();
        Message {
            opcode: opcode,
            payload: encoded
        }
    }
}

#[derive(RustcDecodable, RustcEncodable, PartialEq, Debug, Clone)]
pub struct JsonDictionary {
    dictionary: BTreeMap<String, String>
}

impl JsonDictionary {
    pub fn new() -> JsonDictionary {
        JsonDictionary {
            dictionary: BTreeMap::new()
        }
    }

    pub fn clear(&mut self) {
        self.dictionary.clear();
    }

    pub fn get<T>(&self, key: &str) -> Option<Result<T, json::DecoderError>> where T: Decodable {
        let json = self.dictionary.get(key);
        match json {
            None => None,
            Some(ref json) => Some(json::decode(json))
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.dictionary.contains_key(key)
    }

    pub fn insert<T>(&mut self, key: &str, value: &T)
            -> Result<(), json::EncoderError> where T : Encodable {
        match json::encode(value) {
            Ok(encoded) => {
                self.dictionary.insert(key.to_owned(), encoded);
                Ok(())
            },
            Err(err) => Err(err)
        }
    }

    pub fn remove(&mut self, key: &str) -> bool {
        match self.dictionary.remove(key) {
            Some(_) => true,
            None => false
        }
    }

    pub fn len(&self) -> usize {
        return self.dictionary.len();
    }

    pub fn is_empty(&self) -> bool {
        return self.dictionary.is_empty();
    }
}

#[derive(Clone)]
pub struct Fanout<T> where T : 'static + Send + Sync + Clone {
    broadcast_tx: Sender<T>,
    pub subscribers: Arc<Mutex<Vec<Sender<T>>>>
}

impl<T> Fanout<T>  where T : 'static + Send + Sync + Clone {
    pub fn new() -> Fanout<T> {
        let (broadcast_tx, broadcast_rx) = channel::<T>();
        let subscribers = Arc::new(Mutex::new(Vec::<Sender<T>>::new()));

        let cloned_subscribers = subscribers.clone();
        spawn(move || {
            for message in broadcast_rx.iter() {
                let subscribers_mutex = cloned_subscribers.lock();
                if let Err(err) = subscribers_mutex {
                    panic!("Subscriber mutex gave an error {}", err)
                }
                let mut subscribers = subscribers_mutex.unwrap();
                let mut stale_subscribers_indices = Vec::<usize>::new();
                for (index, subscriber_tx) in subscribers.iter().enumerate() {
                    match subscriber_tx.send(message.clone()) {
                        Ok(_) => {},
                        Err(_) => {
                            stale_subscribers_indices.push(index);
                        }
                    };
                }
                // Prune stale indices
                stale_subscribers_indices.sort();
                for index in stale_subscribers_indices.into_iter().rev() {
                    subscribers.remove(index);
                }
            }
        });

        Fanout {
            broadcast_tx: broadcast_tx,
            subscribers: subscribers
        }
    }

    pub fn subscribe(&mut self) -> Receiver<T> {
        let (tx, rx) = channel::<T>();
        self.subscribers.lock().unwrap().push(tx);
        rx
    }

    pub fn broadcast(&self, message: &T) {
        match self.broadcast_tx.send(message.clone()) {
            Ok(_) => {},
            Err(err) => {
                panic!("Broadcaster has been deallocated {}", err);
            }
        };
    }
}

#[cfg(test)]
mod tests {
    extern crate timebomb;
    use self::timebomb::timeout_ms;
    use super::{Fanout, Message, OpCode};
    use super::super::{PullRequest, User};

    const TIMEOUT: u32 = 1000;

    fn test_payload() -> PullRequest {
        PullRequest {
            id: 111,
            web_url: "http://www.foobar.com".to_owned(),
            from_ref: "abc".to_owned(),
            from_commit: "ffffff".to_owned(),
            title: "A very important PR".to_owned(),
            author: User {
                name: "Aaron Xiao Ming".to_owned(),
                email: "aaron@xiao.ming".to_owned()
            }
        }
    }

    #[test]
    fn it_broadcasts_messages_correctly() {
        let mut fanout = Fanout::<Message>::new();

        let subscriber_one = fanout.subscribe();
        let subscriber_two = fanout.subscribe();

        let expected_message = Message::new(OpCode::OpenPullRequest, &test_payload());
        let expected_message_clone = expected_message.clone();

        fanout.broadcast(&expected_message);

        timeout_ms(move || {
            let message = subscriber_one.recv();
            assert_eq!(expected_message, message.unwrap());
        }, TIMEOUT);


        timeout_ms(move || {
            let message = subscriber_two.recv();
            assert_eq!(expected_message_clone, message.unwrap());
        }, TIMEOUT);
    }

    #[test]
    fn it_does_not_panic_with_dropped_subscribers() {
        let mut fanout = Fanout::<Message>::new();

        let subscriber_one = fanout.subscribe();
        {
            let _subscriber_two = fanout.subscribe(); // will be dropped after this
            assert_eq!(fanout.subscribers.lock().unwrap().len(), 2);
        }

        let expected_message = Message::new(OpCode::OpenPullRequest, &test_payload());

        fanout.broadcast(&expected_message);

        timeout_ms(move || {
            let message = subscriber_one.recv();
            assert_eq!(expected_message, message.unwrap());
        }, TIMEOUT);

        assert_eq!(fanout.subscribers.lock().unwrap().len(), 1);
    }
}

#[cfg(test)]
mod json_dictionary_tests {
    use super::{JsonDictionary};
    use rustc_serialize::{json, Decodable};

    #[derive(RustcDecodable, RustcEncodable, PartialEq, Debug)]
    struct Payload {
        pub payload: String
    }

    #[derive(RustcDecodable, RustcEncodable, PartialEq, Debug)]
    struct OtherPayload {
        pub other: String
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

    fn unwrap_from_json_dictionary<T>(dictionary: &JsonDictionary, key: &str)
         -> T where T : Decodable {
        match dictionary.get::<T>(key) {
            Some(Ok(result)) => result,
            _ => panic!("Unable to unwrap object")
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
