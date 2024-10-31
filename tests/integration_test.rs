use ferdis::server::run_server;
use ferdis::client::send_message;
use ferdis::client::FerdisResponse;
use ferdis::server::ResType;
use std::thread;
use std::time::Duration;

#[test]
fn end_to_end_test() {
    thread::spawn(|| {
        run_server();
    });
    thread::sleep(Duration::from_secs(1));

    match send_message("get my_key".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "NIL");
            assert_eq!(res.res_code, 0);
            assert!(res.message.is_none());
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("set my_key my_value".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "NIL");
            assert_eq!(res.res_code, 0);
            assert!(res.message.is_none());
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("get my_key".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "STR");
            assert_eq!(res.res_code, 0);
            assert!(res.message.is_some());
            assert_eq!(res.message.unwrap(), "my_value");
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("set my_key other_value".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "NIL");
            assert_eq!(res.res_code, 0);
            assert!(res.message.is_none());
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("get my_key".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_code, 0);
            assert_eq!(res.res_type.as_str(), "STR");
            assert!(res.message.is_some());
            assert_eq!(res.message.unwrap(), "other_value");
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("del my_key".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_code, 0);
            assert_eq!(res.res_type.as_str(), "STR");
            assert!(res.message.is_some());
            assert_eq!(res.message.unwrap(), "other_value");
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("get my_key".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "NIL");
            assert_eq!(res.res_code, 0);
            assert!(res.message.is_none());
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("get".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "ERR");
            assert_eq!(res.res_code, 2);
            assert!(res.message.is_some());
            assert_eq!(res.message.unwrap(), "Insufficient arguments");
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("del".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "ERR");
            assert_eq!(res.res_code, 2);
            assert!(res.message.is_some());
            assert_eq!(res.message.unwrap(), "Insufficient arguments");
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("set".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "ERR");
            assert_eq!(res.res_code, 2);
            assert!(res.message.is_some());
            assert_eq!(res.message.unwrap(), "Insufficient arguments");
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("set key".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "ERR");
            assert_eq!(res.res_code, 2);
            assert!(res.message.is_some());
            assert_eq!(res.message.unwrap(), "Insufficient arguments");
        },
        Err(_) => {
            assert!(false);
        }
    }

    match send_message("sadfasdf".to_string()) {
        Ok(res) => {
            assert_eq!(res.res_type.as_str(), "ERR");
            assert_eq!(res.res_code, 1);
            assert!(res.message.is_some());
            assert_eq!(res.message.unwrap(), "Unknown command");
        },
        Err(_) => {
            assert!(false);
        }
    }
}
