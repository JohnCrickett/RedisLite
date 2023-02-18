use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub(crate) struct State {
    pub value: String,
    pub expiration: Option<u128>,
}

pub(crate) type StateMap = Arc<RwLock<HashMap<String, State>>>;
