# rust-http-server

Simple HTTP server in Rust from scratch.

Run:

```bash
cargo run
cargo run -- --directory lol
```

Try:

```bash
curl -i localhost:4221
curl -i localhost:4221/user-agent
curl -i localhost:4221/echo/hello
curl -i localhost:4221/echo -X POST -d "hello"
curl -i localhost:4221/files/poem.txt
curl -i localhost:4221/files/hello.txt -X POST -d "hello"
curl -i localhost:4221/files/hello.txt -X DELETE -d
```
