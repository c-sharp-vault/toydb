use super::{Iter, Range, Store};
use crate::raft;
use crate::utility::{deserialize, serialize};
use crate::Error;

/// A Raft-backed key-value store. The underlying Raft state machine must be
/// generated from Raft::new_state().
pub struct Raft {
    raft: raft::Raft,
}

impl std::fmt::Debug for Raft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Raft")
    }
}

impl Raft {
    /// Creates a new key-value store around a Raft cluster.
    pub fn new(raft: raft::Raft) -> Self {
        Self { raft }
    }

    /// Creates an underlying Raft state machine, which is itself a key-value store.
    pub fn new_state<S: Store>(store: S) -> State {
        State::new(store)
    }
}

impl Store for Raft {
    fn delete(&mut self, key: &[u8]) -> Result<(), Error> {
        self.raft.mutate(serialize(Mutation::Delete(key.to_vec()))?)?;
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        Ok(deserialize(self.raft.read(serialize(Read::Get(key.to_vec()))?)?)?)
    }

    fn iter_prefix(&self, prefix: &[u8]) -> Box<Range> {
        let items: Vec<(Vec<u8>, Vec<u8>)> = deserialize(
            self.raft.read(serialize(Read::GetPrefix(prefix.to_vec())).unwrap()).unwrap(),
        )
        .unwrap();
        Box::new(Iter::from_vec(items))
    }

    fn set(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), Error> {
        self.raft.mutate(serialize(Mutation::Set(key.to_vec(), value))?)?;
        Ok(())
    }
}

/// A state machine mutation
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Mutation {
    /// Deletes a key
    Delete(Vec<u8>),
    /// Sets a key to a value
    Set(Vec<u8>, Vec<u8>),
}

/// A state machine read
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Read {
    /// Fetches a key
    Get(Vec<u8>),
    /// Fetches an array of pairs under a key prefix
    GetPrefix(Vec<u8>),
}

/// The underlying state machine for the store
pub struct State {
    store: Box<dyn Store>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State")
    }
}

impl State {
    pub fn new<S: Store>(store: S) -> Self {
        State { store: Box::new(store) }
    }
}

impl raft::State for State {
    fn mutate(&mut self, command: Vec<u8>) -> Result<Vec<u8>, Error> {
        let mutation: Mutation = deserialize(command)?;
        match mutation {
            Mutation::Delete(key) => {
                info!("Deleting {:?}", key);
                self.store.delete(&key)?;
                Ok(vec![])
            }
            Mutation::Set(key, value) => {
                info!("Setting {:?} to {:?}", key, value);
                self.store.set(&key, value)?;
                Ok(vec![])
            }
        }
    }

    fn read(&self, command: Vec<u8>) -> Result<Vec<u8>, Error> {
        let read: Read = deserialize(command)?;
        match read {
            Read::Get(key) => {
                info!("Getting {:?}", key);
                Ok(serialize(self.store.get(&key)?)?)
            }
            Read::GetPrefix(prefix) => {
                info!("Getting pairs under prefix {:?}", prefix);
                let pairs: Vec<(Vec<u8>, Vec<u8>)> =
                    self.store.iter_prefix(&prefix).collect::<Result<_, Error>>()?;
                Ok(serialize(pairs)?)
            }
        }
    }
}
