use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::spawn;
use std::marker::Send;
use rustc_serialize::{json, Encodable};

#[derive(RustcDecodable, RustcEncodable, Debug, Clone)]
pub enum OpCode {
    OpenPullRequest,
    BuildFound,
    BuildNotFound,
    BuildScheduled,
    BuildFinished { success: bool },
    BuildRunning,
    BuildQueued
}

#[derive(RustcDecodable, RustcEncodable, Debug, Clone)]
pub struct Message {
    pub opcode: OpCode,
    pub payload: String
}

impl Message {
    pub fn make<T>(opcode: OpCode, payload: &T) -> Message where T: Encodable {
        let encoded = json::encode(payload).unwrap();
        Message {
            opcode: opcode,
            payload: encoded
        }
    }
}

pub struct Fanout<T> where T : 'static + Send + Sync + Clone {
    broadcast_tx: Sender<T>,
    subscribers: Arc<Mutex<Vec<Sender<T>>>>
}

impl<T> Fanout<T>  where T : 'static + Send + Sync + Clone {
    pub fn new() -> Fanout<T> {
        let (broadcast_tx, broadcast_rx) = channel::<T>();
        let subscribers = Arc::new(Mutex::new(Vec::<Sender<T>>::new()));

        let cloned_subscribers = subscribers.clone();
        spawn(move || {
            println!("Broadcast loop");
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
