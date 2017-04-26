use std::sync::mpsc::Receiver;
use std::thread::Builder;

use serde::Serialize;
use serde_json;
use ws::{self, WebSocket, Sender, Handler, Message};

/// A WebSocket client connection
struct Client {
    sender: Sender,
}

impl Handler for Client {
    fn on_message(&mut self, message: Message) -> Result<(), ws::Error> {
        debug!("Received message {:?}", msg);
        self.sender.send(message)
    }
}

/// Create a websocket endpoint
pub fn listen<T>(address: &str, receiver: Receiver<T>) -> Result<(), String>
    where T: Serialize + Send + Sync + Clone + 'static
{
    let ws = WebSocket::new(|sender| Client { sender: sender })
        .map_err(|e| e.to_string())?;
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
        .spawn(move || for message in receiver.iter() {
                   let message = serde_json::to_string(&message).unwrap();
                   broadcaster.send(message).unwrap()
               })
        .map_err(|e| e.to_string())?;

    Ok(())
}
