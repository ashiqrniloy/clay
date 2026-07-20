fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

struct Point {
    x: f64,
    y: f64,
}


impl Point {
    fn distance_from_origin(&self) -> f64 {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }
}

// some comment to check what is happening

fn main() {
    let p = Point { x: 3.0, y: 4.0 };
    println!("{}", greet("world"));
    println!("distance: {}", p.distance_from_origin());
}

