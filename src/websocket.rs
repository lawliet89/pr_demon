use std::sync::mpsc::Receiver;
use std::thread::Builder;

use serde::Serialize;
use serde_json;
use ws::{self, Handler, Message, Sender, WebSocket};

/// A WebSocket client connection
struct Client {
    sender: Sender,
}

impl Handler for Client {
    fn on_message(&mut self, message: Message) -> Result<(), ws::Error> {
        debug!("Received message {:?}", message);
        self.sender.send(message)
    }
}

/// Create a websocket endpoint
pub fn listen<T>(address: &str, receiver: Receiver<T>) -> Result<(), String>
where
    T: Serialize + Send + Sync + Clone + 'static,
{
    let ws = WebSocket::new(|sender| Client { sender: sender }).map_err(|e| e.to_string())?;
    let broadcaster = ws.broadcaster();

    let address = address.to_string();
    // TODO: Manage `JoinHandle`s and recover from their panics

    // Start websocket server
    Builder::new()
        .name("websocket".to_string())
        .spawn(move || {
            ws.listen(address)
                .unwrap_or_else(|err| panic!("failed to start websocket listener {}", err));
        })
        .map_err(|e| e.to_string())?;

    // Message sender
    Builder::new()
        .name("websocket_sender".to_string())
        .spawn(move || {
            for message in receiver.iter() {
                let message = serde_json::to_string(&message).unwrap();
                broadcaster.send(message).unwrap()
            }
        })
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json;
    use timebomb::timeout_ms;
    use ws::{connect, CloseCode};

    use super::listen;
    use fanout::{Fanout, Message, OpCode};

    const TIMEOUT: u32 = 2000;

    fn test_payload() -> ::PullRequest {
        ::PullRequest {
            id: 111,
            web_url: "http://www.foobar.com".to_owned(),
            from_ref: "abc".to_owned(),
            from_commit: "ffffff".to_owned(),
            to_ref: "abc".to_owned(),
            to_commit: "ffffff".to_owned(),
            title: "A very important PR".to_owned(),
            author: ::User {
                name: "Aaron Xiao Ming".to_owned(),
                email: "aaron@xiao.ming".to_owned(),
            },
        }
    }

    /// This test is really, really, flaky on Travis
    #[test]
    #[ignore]
    fn websocket_server_is_set_up() {
        let message = Message::new(OpCode::OpenPullRequest, &test_payload()).unwrap();

        let mut fanout = Fanout::<Message>::new();
        let subscriber = fanout.subscribe();

        listen("0.0.0.0:56474", subscriber).unwrap();

        timeout_ms(
            move || {
                connect("ws://0.0.0.0:56474", move |out| {
                    let expected_message = serde_json::to_string(&message).unwrap();
                    fanout.broadcast(message.clone());
                    move |msg| {
                        let msg = format!("{}", msg);
                        assert_eq!(msg, expected_message);
                        out.close(CloseCode::Normal)
                    }
                }).unwrap()
            },
            TIMEOUT,
        );
    }
}
