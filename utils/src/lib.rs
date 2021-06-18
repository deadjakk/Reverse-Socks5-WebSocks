use std::net::{TcpStream,Shutdown};
use websocket::{sender::Writer,receiver::Reader};
use websocket::{OwnedMessage};
use std::io::{Read,Write};
use std::thread;
use std::time::Duration;

pub const TIMEOUT: u64 = 3000;

#[macro_export]
macro_rules! trace {
    ($l: literal) => { if cfg!(trace) { println!("DEBUG:{:?}",$l); } };
    ($l: literal, $e: expr) => { if cfg!(trace) { println!("DEBUG:{:?}-{:?}",$l,$e); } };
}
#[macro_export]
macro_rules! debug {
    ($l: literal) => { if cfg!(debug) { println!("DEBUG:{:?}",$l); } };
    ($l: literal, $e: expr) => { if cfg!(debug) { println!("DEBUG:{:?}-{:?}",$l,$e); } };
}

#[macro_export]
macro_rules! edebug {
    ($l: literal) => { if cfg!(debug) { eprintln!("DEBUG:{:?}",$l); } };
    ($l: literal, $e: expr) => { if cfg!(debug) { eprintln!("DEBUG:{:?}-{:?}",$l,$e); } };
}

macro_rules! shutdown {
    ($sock: ident, $l:literal, $e: expr) => {
        edebug!($l,$e); 
        if let Err(e) = $sock.shutdown(Shutdown::Both){
            edebug!("error shutting down socket:",e);
        }
    };
    ($sock: ident, $l:literal) => {
        edebug!($l); 
        if let Err(e) = $sock.shutdown(Shutdown::Both){
            edebug!("error shutting down socket:",e);
        }
    };
}

pub fn rcopy(stream: &mut TcpStream, mut rstream: Reader<TcpStream>){
    loop{
        for message in rstream.incoming_messages() {
            let message = match message {
                Ok(v) => v,
                Err(e) => {
                    shutdown!(stream,"error unwrapping incoming message from rcopy:",e);
                    if let Err(e) = rstream.shutdown_all(){
                        edebug!("error shutting down reader stream.:",e);
                    }
                    return;
                }
            };
            match message {
                OwnedMessage::Close(_) => {
                    shutdown!(stream,"copied Client disconnected");
                    if let Err(e) = rstream.shutdown_all(){
                        edebug!("error shutting down reader stream:",e);
                    }
                    return;
                }
                OwnedMessage::Binary(data_vec) => {
                    if let Err(e) = stream.write(&data_vec.into_boxed_slice()){
                        shutdown!(stream,"error sending data_vec:",e);
                        if let Err(e) = rstream.shutdown_all(){
                            edebug!("error shutting down reader stream:.",e);
                        }
                        return;
                    }
                }
                _ => {
                    shutdown!(stream,"recv'd something that wasn't binary");
                    if let Err(e) = rstream.shutdown_all(){
                        edebug!("error shutting down reader stream:..",e);
                    }
                    return;
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

pub fn wcopy(stream: &mut TcpStream, mut wstream: Writer<TcpStream>){
    loop{
        let mut buf = [0u8;2048];
        match stream.peek(&mut buf){
            Ok(0) => {
                if let Err(e) = wstream.send_message(&OwnedMessage::Close(None)){
                    edebug!("Error sending message in wcopy:",e); 
                }
                shutdown!(stream,"reading tcp socket has died");
                if let Err(e) = wstream.shutdown_all(){
                    edebug!("error shutting down writer stream:..",e);
                }
                return;
            } 

            Ok(n)=>{
                let mut recv_vec = Vec::new(); 
                for _ in 0..n {
                    recv_vec.push(0u8);
                }

                if let Err(e) = stream.read_exact(&mut recv_vec){
                    if let Err(e) = wstream.send_message(&OwnedMessage::Close(None)){
                        edebug!("Error sending close message in wcopy:",e); 
                    }
                    shutdown!(stream,"error reading exact: ,closing thing",e);
                    if let Err(e) = wstream.shutdown_all(){
                        edebug!("error shutting down writer stream:.",e);
                    }
                    return;
                }
                
                //write to wsocket
                let message = OwnedMessage::Binary(recv_vec);
                if let Err(e) = wstream.send_message(&message){
                    if let Err(e) = wstream.send_message(&OwnedMessage::Close(None)){
                        edebug!("Error sending close message in wcopy:",e); 
                    }
                    shutdown!(stream,"err sending message in wcopy:",e);
                    if let Err(e) = wstream.shutdown_all(){
                        edebug!("error shutting down writer stream:..",e);
                    }
                    return;
                }
            }

            Err(e)=>{
                if let Err(e) = wstream.send_message(&OwnedMessage::Close(None)){
                    edebug!("error sending the close in wcopy:",e);
                }
                shutdown!(stream,"err w/ tcpsocket, closing:",e);
                if let Err(e) = wstream.shutdown_all(){
                    edebug!("error shutting down writer stream:",e);
                }
                return;
            }
        }
        thread::sleep(Duration::from_millis(1));
    }
}




