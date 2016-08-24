use std::sync::mpsc::Receiver;
use std::thread;
use std::time;
use telegram_bot;

use fanout::{Message, OpCode};

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

    thread::spawn(move ||
      for message in subscriber.iter() {
        match message.opcode {
          OpCode::BuildFinished { success: false } => {
            if let Err(err) = api.send_message(room, message.payload, None, None, None, None) {
              println!("{}", err)
            }
            thread::sleep(time::Duration::new(1, 0))
          },
          _ => {} // noop
        }
      }
    );

    Ok(())
  }
}
