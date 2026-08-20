//! Crate doc comment.

use std::collections::HashMap;

const MAX_DEPTH: usize = 128;
static mut COUNTER: u64 = 0;

#[derive(Debug, Clone)]
pub struct Engine<T: Send + 'static> {
    state: HashMap<String, T>,
}

impl<T: Send + 'static> Engine<T> {
    pub fn new() -> Self {
        Self { state: HashMap::new() }
    }

    pub async fn run(&self, input: &str) -> Option<bool> {
        self.state.contains_key(input)
    }
}

fn main() {
    let mut engine = Engine::<bool>::new();
    let items = vec!["one", "two", "three"];
    let message = format!("count={}", items.len());
    println!("{}", message);

    for item in &items {
        if item.len() > 2 {
            continue;
        }
        println!("item: {}", item);
    }

    match engine.run("foo") {
        Some(true) => {}
        _ => {}
    }
}
