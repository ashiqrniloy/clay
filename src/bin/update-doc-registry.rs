use clay::docs::registry::{repository_root, update_generated_registry};

fn main() {
    let root = repository_root();
    match update_generated_registry(&root) {
        Ok(path) => println!("updated {}", path.display()),
        Err(error) => {
            eprintln!("failed to update Clay JS API registry: {error}");
            std::process::exit(1);
        }
    }
}
