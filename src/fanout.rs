use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};
use std::marker::Send;

#[derive(RustcDecodable, RustcEncodable, Debug, Clone)]
pub struct Message {
    message_type: String,
    text: String
}

pub struct Fanout<T> where T : 'static + Send + Sync + Clone {
    broadcast_tx: Sender<T>,
    subscribers: Arc<Mutex<Vec<Sender<T>>>>
}

impl<T> Fanout<T>  where T : 'static + Send + Sync + Clone {
    fn new() -> Fanout<T> {
        let (broadcast_tx, broadcast_rx) = channel::<T>();
        let subscribers = Arc::new(Mutex::new(Vec::<Sender<T>>::new()));

        let cloned_subscribers = subscribers.clone();
        spawn(move || {
            println!("Broadcast loop");
            for message in broadcast_rx.iter() {
                for subscriber_tx in cloned_subscribers.lock().unwrap().iter() {
                    subscriber_tx.send(message.clone());
                }
            }
        });

        Fanout {
            broadcast_tx: broadcast_tx,
            subscribers: subscribers
        }
    }

    fn subscribe(&mut self) -> Receiver<T> {
        let (tx, rx) = channel::<T>();
        self.subscribers.lock().unwrap().push(tx);
        rx
    }

    fn broadcast(&self, message: &T) {
        self.broadcast_tx.send(message.clone());
    }
}
