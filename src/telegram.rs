use std::sync::mpsc::Receiver;
use std::thread;
use telegram_bot;

use fanout::{Message, OpCode};

pub struct TelegramAnnouncer;

impl TelegramAnnouncer {
  pub fn announce_to(token: &str, subscriber: Receiver<Message>, room_id: i64) -> Result<(), String> {
    let api = telegram_bot::Api::from_token(token).unwrap();

    thread::spawn(move ||
      for message in subscriber.iter() {
        match message.opcode {
          OpCode::BuildFinished { success: false } => {
            api.send_message(
              room_id,
              message.payload,
              None, None, None, None
            ).unwrap();
          },
          _ => {} // noop
        }
      }
    );

    Ok(())
  }
}

