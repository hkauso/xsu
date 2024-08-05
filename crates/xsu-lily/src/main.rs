//! Lily CLI
use xsu_dataman::utility;
use xsu_lily::{garden, pack::Pack};

#[tokio::main]
async fn main() {
    let this = garden::Garden::new().await;
    this.init().await;

    // Pack::new(vec!["crates".to_string()], utility::random_id());
    this.stage
        .add_glob(vec!["target/**/*".to_string(), ".git/**/*".to_string()])
        .unwrap();

    let patch = garden::Patch::from_file(
        "name.ext".to_string(),
        "unchanged\nunchanged\nHello World!\nunchanged".to_string(),
        "unchanged\nunchanged\nHello, world!\nunchanged".to_string(),
    );

    // render patch
    for file in patch.render() {
        println!("{file}")
    }

    // ...
    ()
}
