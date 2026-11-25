use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;
use zikaron_net::{exchange, parse, post_json, Ask, Fail, Limits};

fn limits() -> Limits {
    Limits { deadline: Duration::from_secs(10), max_answer: 0 }
}

fn server(serve: fn(TcpStream)) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || serve(s));
        }
    });
    port
}

fn take_hello(s: &mut TcpStream) {
    let mut buf = [0u8; 4096];
    let _ = s.read(&mut buf);
}

fn plain_http(mut s: TcpStream) {
    take_hello(&mut s);
    let _ = s.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let _ = s.flush();
    std::thread::sleep(Duration::from_millis(200));
}

fn closes_after_hello(mut s: TcpStream) {
    take_hello(&mut s);
    let _ = s.shutdown(std::net::Shutdown::Both);
}

fn garbage(mut s: TcpStream) {
    take_hello(&mut s);
    let _ = s.write_all(&[0xff; 64]);
    let _ = s.flush();
    std::thread::sleep(Duration::from_millis(200));
}

const REQUEST: &[u8] = b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";

#[test]
fn a_peer_that_does_not_speak_tls_is_the_handshake() {
    for (form, serve) in [("plainHttp", plain_http as fn(TcpStream)), ("closesAfterHello", closes_after_hello), ("garbage", garbage)] {
        let port = server(serve);
        let t = parse(&format!("https://127.0.0.1:{port}/")).expect("address");
        match exchange(&t, REQUEST, &limits()) {
            Err(Fail::Handshake(said)) => assert!(!said.is_empty(), "{form}: the handshake failure says why"),
            other => panic!("{form}: the handshake member, not {:?}", other.err()),
        }
        for how in [Ask::Once, Ask::Read] {
            match post_json(&t, b"{}", &limits(), how) {
                Err(Fail::Handshake(_)) => {}
                other => panic!("{form} {how:?}: the handshake member, not {:?}", other.err()),
            }
        }
    }
}
