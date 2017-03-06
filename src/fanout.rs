use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::spawn;
use std::marker::Send;
use rustc_serialize::{json, Encodable};

#[derive(RustcDecodable, RustcEncodable, PartialEq, Debug, Clone)]
pub enum OpCode {
    OpenPullRequest,
    BuildFound,
    BuildNotFound,
    BuildScheduled,
    BuildFinished { success: bool },
    BuildRunning,
    BuildQueued,
    Custom { payload: String },
}

#[derive(RustcDecodable, RustcEncodable, PartialEq, Debug, Clone)]
pub struct Message {
    pub opcode: OpCode,
    pub payload: String,
}

impl Message {
    pub fn new<T>(opcode: OpCode, payload: &T) -> Message
        where T: Encodable
    {
        let encoded = json::encode(payload).unwrap();
        Message {
            opcode: opcode,
            payload: encoded,
        }
    }
}

#[derive(Clone)]
pub struct Fanout<T>
    where T: 'static + Send + Sync + Clone
{
    broadcast_tx: Sender<T>,
    pub subscribers: Arc<Mutex<Vec<Sender<T>>>>,
}

impl<T> Fanout<T>
    where T: 'static + Send + Sync + Clone
{
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
                    if subscriber_tx.send(message.clone()).is_err() {
                        stale_subscribers_indices.push(index);
                    }
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
            subscribers: subscribers,
        }
    }

    pub fn subscribe(&mut self) -> Receiver<T> {
        let (tx, rx) = channel::<T>();
        self.subscribers.lock().unwrap().push(tx);
        rx
    }

    pub fn broadcast(&self, message: &T) {
        if let Err(err) = self.broadcast_tx.send(message.clone()) {
            panic!("Broadcaster has been deallocated {}", err);
        }
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
            to_ref: "abc".to_owned(),
            to_commit: "ffffff".to_owned(),
            title: "A very important PR".to_owned(),
            author: User {
                name: "Aaron Xiao Ming".to_owned(),
                email: "aaron@xiao.ming".to_owned(),
            },
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
                   },
                   TIMEOUT);


        timeout_ms(move || {
                       let message = subscriber_two.recv();
                       assert_eq!(expected_message_clone, message.unwrap());
                   },
                   TIMEOUT);
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
                   },
                   TIMEOUT);

        assert_eq!(fanout.subscribers.lock().unwrap().len(), 1);
    }
}
