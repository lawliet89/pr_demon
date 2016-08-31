use std::sync::mpsc::Receiver;
use std::thread;
use std::time;
use telegram_bot;
use rustc_serialize::{json, Decodable};

use fanout::{Message, OpCode, JsonDictionary};

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct TelegramCredentials {
    pub enabled: bool,
    pub api_token: String,
    pub room: i64
}

impl TelegramCredentials {
    pub fn announce_from(&self, subscriber: Receiver<Message>) -> Result<(), String> {
        let api = match telegram_bot::Api::from_token(self.api_token.as_str()) {
            Ok(x) => x,
            Err(err) => return Err(format!("{}", err))
        };

        let room = self.room;

        thread::spawn(move || {
            let telegram_sleep_duration = time::Duration::new(1, 0);
            for message in subscriber.iter() {
                match message.opcode {
                    OpCode::Custom { payload: ref custom_payload }
                        if custom_payload == "Bitbucket::Comment::Update"
                            || custom_payload == "Bitbucket::Comment::Post" => {
                        // panic if payload cannot be deserialized
                        let dictionary: JsonDictionary = json::decode(&message.payload).unwrap();
                        // should panic if the deserialization failed
                        let build = Self::unwrap_from_json_dictionary::<::BuildDetails>(&dictionary, "build").unwrap();
                        if build.state != ::BuildState::Finished  {
                            continue;
                        }

                        // should panic if the deserialization failed
                        let pr = Self::unwrap_from_json_dictionary::<::PullRequest>(&dictionary, "pr").unwrap();

                        let status_text = match build.status_text {
                            Some(text) => text,
                            None => "".to_owned()
                        };
                        let message_text = format!("⚠ Tests for Pull Request #{} has failed\n{}\n{}\nBy {}\n{}\n{}",
                            pr.id, status_text, pr.title, pr.author.name, pr.web_url, build.web_url);

                        Self::send_message(&api, room, message_text);
                        thread::sleep(telegram_sleep_duration);
                    }
                    _ => {} // noop
                };
            }
        });
        Ok(())
    }

    fn send_message(api: &telegram_bot::Api, room: i64, message: String) {
        if let Err(err) = api.send_message(room, message, None, None, None, None) {
            println!("{}", err)
        }
    }

    fn unwrap_from_json_dictionary<T>(dictionary: &JsonDictionary, key: &str)
         -> Result<T, ()> where T : Decodable {
        match dictionary.get::<T>(key) {
            Some(Ok(result)) => Ok(result),
            _ => Err(())
        }
    }
}
