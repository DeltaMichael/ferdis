use ferdis::server::run_server;
use ferdis::client::send_message;
use std::env;

fn main() {
    let mut args: Vec<String> = env::args().collect();
    args.remove(0);
    if args.len() == 0 {
        run_server();
        return;
    }
    if args[0] == "client" {
        args.remove(0);
        let req = args.join(" ");
        let response = send_message(req);
        println!("{:?}", response);
    } else {
        panic!("Wrong arguments");
    }
}
