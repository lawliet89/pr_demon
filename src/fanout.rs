use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::Arc;
use std::sync::RwLock;
use std::thread::{spawn, JoinHandle};
use std::marker::Send;

pub struct Fanout<T> where T : 'static + Send + Sync + Clone {
    sender: Sender<T>,
    subscribers: Arc<RwLock<Vec<Sender<T>>>>
}

impl<T> Fanout<T>  where T : 'static + Send + Sync + Clone {
    fn new() -> Fanout<T> {
        let (sender, receiver) = channel::<T>();
        let subscribers: Vec<Sender<T>> = vec![];
        let lock = RwLock::new(subscribers);
        let arc = Arc::new(lock);

        let clone_arc = arc.clone();
        spawn(move || {
            loop {
                match receiver.recv() {
                    Ok(payload) => {
                        let lock = *clone_arc.read();
                        match lock {
                            Ok(subscribers) => {},
                            Err(_) => break
                        }
                    },
                    Err(_) => break
                };
            }
        });

        Fanout {
            sender: sender,
            subscribers: arc
        }
    }

    // fn subscribe(&mut self) -> Receiver<T> {
    //     let (tx, rx) = channel::<T>();
    //     self.subscribers.push(tx);
    //     rx
    // }

    fn broadcast() {

    }
}
