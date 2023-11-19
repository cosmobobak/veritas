use crate::{NAME, VERSION, timemgmt::Limits};

/// The main loop of the Universal Game Interface (UGI).
pub fn main_loop() {
    let (tx, rx) = std::sync::mpsc::channel();
    let stdin_reader_thread = std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();
        let mut line = String::new();
        while std::io::BufRead::read_line(&mut stdin, &mut line).is_ok() {
            tx.send(line.clone()).unwrap();
            line.clear();
        }
    });

    let version_extension = if cfg!(feature = "final-release") { "" } else { "-dev" };
    println!("{NAME} {VERSION}{version_extension} by Cosmo");

    loop {
        std::io::Write::flush(&mut std::io::stdout()).expect("couldn't flush stdout");
        let Ok(line) = rx.recv() else {
            break;
        };
        let input = line.trim();

        match input {
            "\n" | "\r\n" | "" => continue,
            "quit" => break,
            "isready" => println!("readyok"),
            "ugi" => {
                println!("id name {NAME} {VERSION}{version_extension}");
                println!("id author Cosmo");
                println!("ugiok");
            }
            go if go.starts_with("go") => {
                let limits: Limits = if let Ok(limits) = go.trim_start_matches("go").trim().parse() {
                    limits
                } else {
                    println!("info string invalid go command");
                    continue;
                };
            }
            unknown => println!("info string unknown command: {unknown}"),
        }
    }
}