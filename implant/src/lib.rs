#![forbid(unsafe_code)]
use std::net::{TcpStream, SocketAddr, SocketAddrV4, SocketAddrV6, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::{thread};
use std::time::Duration;
use websocket::client::{ClientBuilder};
use websocket::{receiver::Reader};
use websocket::{OwnedMessage};


use utils::{wcopy,rcopy,debug,edebug,TIMEOUT};

/// Version of socks
const SOCKS_VERSION: u8 = 0x05;

const RESERVED: u8 = 0x00;

#[derive(Clone,Debug, PartialEq)]
pub struct User {
    pub username: String,
    password: String
}

/// Possible SOCKS5 Response Codes
#[derive(Debug)]
enum ResponseCode {
    Success = 0x00,
}

/// DST.addr variant types
#[derive(PartialEq)]
#[derive(Debug)]
enum AddrType {
    V4 = 0x01,
    Domain = 0x03,
    V6 = 0x04,
}

impl AddrType {
    /// Parse Byte to Command
    fn from(n: usize) -> Option<AddrType> {
        match n {
            1 => Some(AddrType::V4),
            3 => Some(AddrType::Domain),
            4 => Some(AddrType::V6),
            _ => None
        }
    }
}

/// SOCK5 CMD Type
#[derive(Debug)]
enum SockCommand {
    Connect = 0x01,
    Bind = 0x02,
    UdpAssosiate = 0x3
}

impl SockCommand {
    /// Parse Byte to Command
    fn from(n: usize) -> Option<SockCommand> {
        match n {
            1 => Some(SockCommand::Connect),
            2 => Some(SockCommand::Bind),
            3 => Some(SockCommand::UdpAssosiate),
            _ => None
        }
    }
}

/// Client Authentication Methods
pub enum AuthMethods {
    /// No Authentication
    NoAuth = 0x00,
}

pub struct Client {
    ip: String,
    port: u16,
}

impl Client {
    pub fn new(port: u16,  ip: &str) -> Result<Self,Box<dyn std::error::Error>> {
        Ok( Client{
            ip: ip.to_string(),
            port,
        })
    }

    pub fn serve(&mut self) -> Result<(),Box<dyn std::error::Error>> {
        loop {
            let client = match ClientBuilder::new(&format!("ws://{}:{}",self.ip,self.port)){
                Ok(client)=>{
                    match client.add_protocol("rust-websocket")
                    .connect_insecure(){
                        Ok(c)=>{
                            c
                        }
                        Err(e)=>{
                            edebug!("error connecting..",e);
                            thread::sleep(Duration::from_millis(1000));
                            continue;
                        }
                    
                    }
                }

                Err(e)=> {
                    edebug!("error connecting",e);
                    continue;
                }
            };

            debug!("successfully connected");
            let (mut receiver, mut sender) = match client.split() {
                Ok(v) => v,
                Err(e)=>{
                    edebug!("error mapping address to a socket addr:",e);
                    //return;
                    continue;
                } 
            };

            let mut nclient = SOCKClient::new();
            debug!("+");

            if let Err(e) = receiver.set_read_timeout(Some(Duration::from_millis(TIMEOUT))){
                edebug!("error setting client read timeout",e);
            }

            let message = match receiver.recv_message(){
                Ok(m)=>m,
                Err(e)=>{
                    edebug!("err receiving message:",e);
                    if let Err(e) = sender.send_message(&OwnedMessage::Close(None)){
                        edebug!("err sending close message:",e);
                    }
                    return Err(From::from(""));
                }
            };

            if let Err(e) = receiver.set_read_timeout(None){
                edebug!("error setting client read timeout back",e);
            }

            debug!("!RECEIVED MSG!");

            // Thread handoff
            thread::spawn(move || {
                debug!("received during init:", message);
                let data = match message {
                    OwnedMessage::Binary(data) => {
                        debug!("received binary data:", &data);
                        data
                    } 
                    _ => {
                        edebug!("received something other than binary");
                        return;
                    }
                };

                nclient.socks_version = data[0];
                nclient.auth_nmethods = data[1];

                // Handle SOCKS4 requests
                if data[0] != SOCKS_VERSION {
                    edebug!("closing, unsupported socks version");
                    if let Err(e) = sender.send_message(&OwnedMessage::Close(None)){
                        edebug!("error sending close message",e);
                    }
                    return;
                }
                // Valid SOCKS5
                else {
                    // Authenticate w/ client
                    let mut msg_vec = Vec::new();
                    msg_vec.push(SOCKS_VERSION);
                    msg_vec.push(AuthMethods::NoAuth as u8);
                    if let Err(e) = sender.send_message(&OwnedMessage::Binary(msg_vec)){
                        edebug!("error responding to initial socks connection:",e);
                    }
                    let req = match SOCKSReq::from_wstream(&mut receiver){
                        Ok(req) => req,
                        Err(e) => {
                            edebug!("error parsing connection from ws stream:",e);
                            return;
                        }
                    };

                    match req.command { 
                        SockCommand::Connect => {
                            
                            let sock_addr = match addr_to_socket(&req.addr_type, &req.addr, req.port){
                                Ok(v) => v,
                                Err(e)=>{
                                    edebug!("error mapping address to a socket addr:",e);
                                    return;
                                } 
                            };

                            let target = match TcpStream::connect(&sock_addr[..]){
                                Ok(v) => v,
                                Err(e)=>{
                                    edebug!("error connecting to requested destination:",e);
                                    return;
                                } 
                            };

                            let mut ok_vec = Vec::new();
                            ok_vec.push(SOCKS_VERSION);
                            ok_vec.push(ResponseCode::Success as u8);
                            ok_vec.push(RESERVED);
                            ok_vec.push(1);
                            ok_vec.push(127);
                            ok_vec.push(0);
                            ok_vec.push(0);
                            ok_vec.push(1);
                            ok_vec.push(0);
                            ok_vec.push(0);

                            if let Err(e) = sender.send_message(&OwnedMessage::Binary(ok_vec)){
                                edebug!("error sending the connect response",e);
                                return;
                            }

                            let mut inbound_in   = match target.try_clone() {
                                Ok(v) => v,
                                Err(e)=>{
                                    edebug!("error cloning inbound socket:",e);
                                    return;
                                } 
                            };
                            let mut inbound_out  = match target.try_clone(){
                                Ok(v) => v,
                                Err(e)=>{
                                    edebug!("error cloning inbound socket:",e);
                                    return;
                                } 
                            };

                            debug!("it's cloning time!");
                            thread::spawn(move || {
                                wcopy(&mut inbound_out, sender);
                            });

                             //Upload Thread
                            thread::spawn(move || {
                                rcopy(&mut inbound_in, receiver);
                            });
                            thread::sleep(Duration::from_millis(1));
                        },
                        SockCommand::Bind => { },
                        SockCommand::UdpAssosiate => { },
                    }
                } // else
            });
            }
    } // loop
} // serve

pub struct SOCKClient {
    auth_nmethods: u8,
    socks_version: u8
}

impl SOCKClient {
    /// Create a new SOCKClient
    pub fn new( ) -> Self {
        SOCKClient {
            auth_nmethods: 0,
            socks_version: 0,
        }
    }
}

/// Convert an address and AddrType to a SocketAddr
fn addr_to_socket(addr_type: &AddrType, addr: &[u8], port: u16) -> Result<Vec<SocketAddr>,Box<dyn std::error::Error>> {
    match addr_type {
        AddrType::V6 => {
            edebug!("IPV6 address received",addr); 
            let new_addr = (0..8).map(|x| {
                (u16::from(addr[(x * 2)]) << 8) | u16::from(addr[(x * 2)])
            }).collect::<Vec<u16>>();
            Ok(vec![SocketAddr::from(
                SocketAddrV6::new(
                    Ipv6Addr::new(
                        new_addr[0], new_addr[1], new_addr[2], new_addr[3], new_addr[4], new_addr[5], new_addr[6], new_addr[7]), 
                    port, 0, 0)
            )])
        },
        AddrType::V4 => {
            Ok(vec![SocketAddr::from(SocketAddrV4::new(Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]), port))])
        },
        AddrType::Domain => {
            let mut domain = String::from_utf8_lossy(&addr[..]).to_string();
            domain.push_str(&":");
            domain.push_str(&port.to_string());
            let domain_sock_addrs = match domain.to_socket_addrs(){
                Ok(v) => v,
                Err(e) => {
                    edebug!("couldn't parse address from provided address data:",e);
                    return Err(From::from(""));
                }
            };
            let domain = domain_sock_addrs.collect();

            Ok(domain)
        }

    }
}

/// Proxy User Request
struct SOCKSReq {
    pub version: u8,
    pub command: SockCommand,
    pub addr_type: AddrType,
    pub addr: Vec<u8>,
    pub port: u16
}

impl SOCKSReq {
    fn from_wstream(stream: &mut Reader<TcpStream>) -> Result<Self,Box<dyn std::error::Error>> {
        // Read a byte from the stream and determine the version being requested
        let message = match stream.recv_message() {
            Ok(v) => v,
            Err(e) => {
                edebug!("error getting connect message from stream:",e);
                return Err(From::from("")); 
            }
        };
        
        let data = match message {
            OwnedMessage::Binary(data) => {
                debug!("received binary data", &data);
                data
            } 
            _ => {
                debug!("received something other than binary");
                return Err(From::from(""));
            }
        };

        if data[0] != SOCKS_VERSION {
            edebug!("err: socks version does not match");
        }

        // Get command
        let mut command: SockCommand = SockCommand::Connect;
        match SockCommand::from(data[1] as usize) {
            Some(com) => {
                command = com;
            },
            None => {
                edebug!("incorrect socks command");
            }
        };

        // DST.address
        let mut addr_type: AddrType = AddrType::V6;
        match AddrType::from(data[3] as usize) {
            Some(addr) => {
                debug!("determined addr type",addr);
                addr_type = addr;
            },
            None => {
                edebug!("addr type incorrect");
            }
        };

        let offset;
        // Get Addr from addr_type and stream
        let addr: Result<Vec<u8>,Box<dyn std::error::Error>> = match addr_type {
            AddrType::Domain => {
                let dlen = data[4];
                let domain = data.get(5..(dlen+5) as usize).expect("domain length incorrect");
                
                offset = dlen+5;
                // println!("domain parsed: {:?}",std::str::from_utf8(&domain.to_vec()));
                Ok(domain.to_vec())
            },
            AddrType::V4 => {
                let addr = data.get(4..8).expect("v4 addr  length incorrect");
                offset = 8;
                Ok(addr.to_vec())
            },
            AddrType::V6 => {
                let addr = data.get(4..19).expect("v6 addr length incorrect");
                offset = 19;
                Ok(addr.to_vec())
            }
        };

        let addr = addr?;

        // read DST.port
        let port = data.get(offset as usize..(offset+2) as usize).expect("port ? length incorrect");

        // Merge two u8s into u16
        let port = (u16::from(port[0]) << 8) | u16::from(port[1]);

        // Return parsed request
        Ok(SOCKSReq {
            version: SOCKS_VERSION,
            command,
            addr_type,
            addr,
            port
        })
    }

}


