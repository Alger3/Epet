use std::path::Path;

use epet_desktop_lib::package::{PackageSummary, load_epet};

fn main() {
    let mut arguments = std::env::args().skip(1);
    let path = arguments
        .next()
        .expect("usage: inspect_epet <package.epet> [expected-sha256]");
    let expected = arguments.next();
    let package = load_epet(Path::new(&path), expected.as_deref())
        .unwrap_or_else(|error| panic!("invalid .epet package: {error}"));
    println!(
        "{}",
        serde_json::to_string_pretty(&PackageSummary::from(&package))
            .expect("serialize package summary")
    );
}
