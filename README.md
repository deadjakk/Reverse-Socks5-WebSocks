# Reverse-Socks5-WebSocks

RustySmuggler is a reverse SOCKS5 proxy tunneled over websockets.  

Very similar to [another project](https://github.com/deadjakk/RustPivot)
I dropped here but was written instead using websockets,
placing it here for now, and may merge the two projects later.  

### Overview  

In short, you place the server on an internet-accessible host which will
listen for a reverse websockets connection from the client binary placed
inside of a network.  
Then from the internet accessible host, a user can proxy through the "frontend"
listening port to access hosts on the internal network.  

### Usage  

```
server 0.1.0

USAGE:
    server -b <backend> -f <frontend>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -b <backend>         Socket address for external WS listener, the one to which the client will connect
    -f <frontend>        Socket address for front end proxy, the one to which the proxychains/proxy client might connect
```

#### Running the server   
1. Build the server (see building section(s))  
2. `./server -b 0.0.0.0:3030 -f 0.0.0.0:2020` 


##### Tips 

- Use IP addresses instead of domain names if issues are encountered
- Use iptables or ssh forwarding to prevent unauthorized access to the client-side (`-f`) port

#### Running the client (implant) 
1. First edit the 'IP' and 'PORT' variables in client/src/config.rs to point to
your server's -b address  
2. Build the client (see building section(s))
3. run the client with `./client`, or in windows: `client.exe`  

#### Using the proxy  
Point your browser or proxychains.conf to match that of your server's `-f` value  

## Building
This tool requires that rust be installed:  
click [here](https://www.rust-lang.org/tools/install) for information on that. 

### Building 

You should be able to get away with simply running:  
`cargo b --release`   
The compiled binaries should be in ./target/release/server and ./target/release/client.  


### Building with Debug Output:  
For fear of having anyone accidently compiling with debug, thus including a ton
of potential IoCs and literals, I made it more annoying by requiring that a compiler flag
be included:  
`RUSTFLAGS='--cfg debug' cargo build`  
or in windows you can use one of the following:  
Windows (psh):  
`$env:RUSTFLAGS='--cfg debug' cargo build --bin client`  
Windows (cmd):  
`set RUSTFLAGS='--cfg debug' cargo build --bin client`  

P.S. If you would like to get the binary even smaller,
[this repo](https://github.com/johnthagen/min-sized-rust)
is a great guide for that sort of thing.  
