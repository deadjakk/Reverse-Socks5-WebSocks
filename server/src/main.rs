extern crate websocket;
use chrono::{DateTime,Local};
use std::thread;
use websocket::sync::Server;
use std::time::{SystemTime,Duration};
use std::sync::{mpsc::channel};
use websocket::{sender::Writer,receiver::Reader,OwnedMessage};
use std::net::{Shutdown, TcpStream, TcpListener};
use utils::{wcopy,rcopy,edebug,TIMEOUT};

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Socket address for front end proxy, the one to which proxychains might connect
    #[structopt(short)]
    frontend: String,

    /// Socket address for front end proxy, the one to which the client will connect
    #[structopt(short)]
    backend: String,
}

fn main(){
    let args = Opt::from_args();

    let frontend = args.frontend;
    let backend  = args.backend;

    let (front_t, front_r)  = channel();   


    thread::spawn(move || {
    loop {
        let listener = match TcpListener::bind(&frontend){
            Ok(l)=>l,
            Err(e) =>{
                eprintln!("err rebinding: {:?}",e);
                thread::sleep(Duration::from_millis(TIMEOUT));
                continue;
            }

        };
        thread::sleep(Duration::from_millis(1));
        let datetime : DateTime<Local> = SystemTime::now().into();
        match listener.accept(){
            Ok((fstream,addr)) => {
                println!("{}|frontend connection|{}", datetime.format("%m-%d-%y|%T"), addr);
                if let Err(e) = front_t.send((fstream,addr)){
                    edebug!("error sending frontend sock to channel",e);
                }
            } 
            _ => (),
        } 
    } 
    });

    loop{
	let server = Server::bind(&backend).unwrap();

    // New request from client socket (-b)
	for request in server.filter_map(Result::ok) {

        // Check for pending front-end socket
        match front_r.recv_timeout(Duration::from_millis(TIMEOUT)){
            Ok((mut fstream,_addr)) => { 
                thread::spawn(move || {

                    if !request.protocols().contains(&"rust-websocket".to_string()) {
                        request.reject().unwrap();
                        return;
                    }
                    match request.use_protocol("rust-websocket").accept(){
                        Ok(client) => {
                            let ip = client.peer_addr().unwrap();

                            match client.split(){
                                Ok((receiver,sender)) => {
                                    let datetime : DateTime<Local> = SystemTime::now().into();
                                    println!("{}|proxy connection  |{}", datetime.format("%m-%d-%y|%T"), ip);
                                    handle_streams(&mut fstream, sender, receiver);
                                }
                                Err(e) => {
                                    eprintln!("error splitting websockets:{:?}",e);
                                }
                            }
                        }
                        Err(e) => {
                                eprintln!("error with receiving client:{:?}",e);
                        }
                    }
                thread::sleep(Duration::from_millis(1));
                });

            } // OK


            Err(_) => {
                // no frontend clients are currently available
                let datetime : DateTime<Local> = SystemTime::now().into();
                println!("{}|no current connection|closing", datetime.format("%m-%d-%y|%T"));

                match request.use_protocol("rust-websocket").accept(){
                    Ok(client) => {

                        match client.split(){
                            Ok((receiver,mut sender)) => {
                                if let Err(e) = sender.send_message(&OwnedMessage::Close(None)){
                                    edebug!("error sending close message via bstream_t",e); 
                                }
                                if let Err(e) = receiver.shutdown_all(){
                                    edebug!("error shutting sockets:",e); 
                                }
                            }
                            Err(e) => {
                                eprintln!("error splitting websockets:{:?}",e);
                            }
                        }
                    }
                    Err(e) => {
                            eprintln!("error with receiving client:{:?}",e);
                    }
                }
            }
        }

	}
    }
}

fn handle_streams(fstream: &mut TcpStream, mut bstream_t: Writer<TcpStream>, bstream_r: Reader<TcpStream>) {

    // Copy it all
    let mut inbound_in   = match fstream.try_clone(){
        Ok(s)=>s, 
        Err(e)=>{
            edebug!("error cloning socks",e); 
            if let Err(e) = fstream.shutdown(Shutdown::Both){
                edebug!("error sending closing fstream",e); 
            }
            if let Err(e) = bstream_t.send_message(&OwnedMessage::Close(None)){
                edebug!("error sending close message via bstream_t",e); 
            }
            if let Err(e) = bstream_r.shutdown_all(){
                edebug!("error shutting down bstream_r sock",e); 
            }
            return;
        }
    };
    let mut inbound_out  = match fstream.try_clone(){
        Ok(s)=>s, 
        Err(e)=>{
            edebug!("error cloning socks",e); 
            if let Err(e) = fstream.shutdown(Shutdown::Both){
                edebug!("error sending closing fstream .",e); 
            }
            if let Err(e) = bstream_t.send_message(&OwnedMessage::Close(None)){
                edebug!("error sending close message via bstream_t .",e); 
            }
            if let Err(e) = bstream_r.shutdown_all(){
                edebug!("error shutting down bstream_r sock",e); 
            }
            return;
        }
    };

    // if alive, copy socks together in new threads
    thread::spawn(move || {
        wcopy(&mut inbound_out, bstream_t);
    });

    // Upload Thread
    thread::spawn(move || {
        rcopy(&mut inbound_in, bstream_r);
    });

}
