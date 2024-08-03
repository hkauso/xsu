/// Markdown manager
pub mod config;

pub fn transform(cnf: &config::Config, mut input: String) -> String {
    for (from, to) in cnf.map.iter() {
        input = input.replace(from, &to);
    }

    input
}
