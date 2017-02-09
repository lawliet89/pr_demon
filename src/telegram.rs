use std::sync::mpsc::Receiver;
use std::thread;
use std::time;

use telebot::bot;
use tokio_core::reactor::Core;
use futures::stream::Stream;
use futures::Future;
use rustc_serialize::{json, Decodable};

use fanout::{Message, OpCode};
use json_dictionary::JsonDictionary;

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
pub struct TelegramCredentials {
    pub enabled: bool,
    pub api_token: String,
    pub room: i64,
}

impl TelegramCredentials {
    pub fn announce_from(&self, subscriber: Receiver<Message>) -> Result<(), String> {
        let mut lp = Core::new().expect("Unable to create Tokio core reactor");

        let api = bot::RcBot::new(lp.handle(), self.api_token.as_str())
                   .update_interval(200);
        let room = self.room;

        thread::spawn(move || {
            let telegram_sleep_duration = time::Duration::new(1, 0);
            for message in subscriber.iter() {
                match message.opcode {
                    OpCode::Custom { payload: ref custom_payload } if custom_payload == "Bitbucket::Comment::Update" ||
                                                                      custom_payload == "Bitbucket::Comment::Post" => {
                        // panic if payload cannot be deserialized
                        let dictionary: JsonDictionary = json::decode(&message.payload).unwrap();
                        // should panic if the deserialization failed
                        let build = Self::unwrap_from_json_dictionary::<::BuildDetails>(&dictionary, "build").unwrap();
                        if build.state != ::BuildState::Finished || build.status == ::BuildStatus::Success {
                            continue;
                        }

                        // should panic if the deserialization failed
                        let pr = Self::unwrap_from_json_dictionary::<::PullRequest>(&dictionary, "pr").unwrap();

                        let status_text = build.status_text
                            .as_ref()
                            .map_or_else(|| "".to_string(), |s| s.to_string());

                        let message_text = format!("⚠ Tests for Pull Request #{} have failed\n{}\n{}\nBy {}\n{}\n{}",
                                                   pr.id,
                                                   status_text,
                                                   pr.title,
                                                   pr.author.name,
                                                   pr.web_url,
                                                   build.web_url);

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
            error!("{}", err)
        }
    }

    fn unwrap_from_json_dictionary<T>(dictionary: &JsonDictionary, key: &str) -> Result<T, ()>
        where T: Decodable
    {
        match dictionary.get::<T>(key) {
            Some(Ok(result)) => Ok(result),
            _ => Err(()),
        }
    }
}
