extern crate philipshue;

use self::philipshue::bridge;
use self::philipshue::errors::{HueError, HueErrorKind, BridgeError};

use std::thread;
use std::time::Duration;
use std::fs::File;
use std::io;
use std::io::{Read, Write};

const USER_FILE_NAME: &'static str = ".hue-user";

pub fn start(ip: String) -> bridge::Bridge {
    let user_result = get_user();

    match user_result {
        Some(user) => {
            connect_to_bridge(ip, user)
        }
        None => {
            if let Some(user) = register_user(&ip) {
                connect_to_bridge(ip, user)
            } else {
                panic!("Failed to register user!");
            }
        }
    }


}

fn register_user(ip: &String) -> Option<String> {
    let final_user: String;

    loop {
        match bridge::register_user(&ip, "cam-to-hue#linux") {
            Ok(user) => {
                println!("Hue user registered: {}, on IP: {}", user, ip);
                write_user(&user).unwrap();
                final_user = user;
                break;
            }
            Err(HueError(HueErrorKind::BridgeError { error: BridgeError::LinkButtonNotPressed, .. }, _)) => {
                println!("Please, press the link on the bridge. Retrying in 5 seconds");
                thread::sleep(Duration::from_secs(5));
            }
            Err(e) => {
                println!("Unexpected error occured: {}", e);
                return None;
            }
        }
    }

    Some(final_user)
}

fn get_user() -> Option<String> {
    let f = File::open(USER_FILE_NAME);
    match f {
        Ok(mut f) => {
            let mut s = String::new();
            let result = f.read_to_string(&mut s);

            match result {
                Ok(_) => Some(s),
                Err(_) => None
            }
        }
        Err(_) => None
    }
}

fn write_user(user: &String) -> Result<(), io::Error> {
    let mut f = try!(File::create(USER_FILE_NAME));
    let result = f.write_all(user.as_bytes());

    match result {
        Ok(_) => Ok(()),
        Err(err) => Err(err)
    }
}

fn connect_to_bridge(ip: String, user: String) -> bridge::Bridge {
    bridge::Bridge::new(ip, user)
}
