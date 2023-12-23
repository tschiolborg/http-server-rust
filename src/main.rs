use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};

fn handle_request(stream: TcpStream) {
    let reader = BufReader::new(&stream);

    let request: Vec<_> = reader
        .lines()
        .map(|x| x.unwrap())
        .take_while(|x| !x.is_empty())
        .collect();
    println!("{:?}", request);

    let start: Vec<_> = request[0].split(" ").collect();
    let path = start[1];

    let mut writer = BufWriter::new(stream);
    match path {
        "/" => writer.write_all(b"HTTP/1.1 200 OK\r\n\r\n").unwrap(),
        _ => writer.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap(),
    };
}

fn main() {
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");
                handle_request(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
