use crate::database::Database;
use core::str;
use libc::*;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

const CLEAR_SCREEN: &str = "\x1B[2J\x1B[H";
const BOLD: &str = "\x1B[1m";
const RESET: &str = "\x1B[0m";
const PINK: &str = "\x1B[35m";
const CYAN: &str = "\x1B[36m";
const HEARTBEAT_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(5);
const TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(5);

pub fn create_raw_socket(port: u16) -> Result<i32, Box<dyn Error>> {
    unsafe {
        // Create a socket
        // AF_INET specifies the IPv4 address fam
        // SOCK_STREAM indicates that the socket will use TCP
        // 0 is default for TCP
        let socket_fd = socket(AF_INET, SOCK_STREAM, 0);

        if socket_fd < 0 {
            eprintln!("Failed to create socket");
            std::process::exit(1);
        }

        // Set socket options
        let option_val: i32 = 1;
        if setsockopt(
            socket_fd,
            SOL_SOCKET,
            SO_REUSEADDR,
            &option_val as *const _ as *const c_void,
            std::mem::size_of_val(&option_val) as u32,
        ) < 0
        {
            eprintln!("Failed to set socket options");
            std::process::exit(1);
        }

        // Bind socket to address
        let address = sockaddr_in {
            sin_family: AF_INET as u16,
            sin_port: htons(port),
            sin_addr: in_addr { s_addr: INADDR_ANY },
            sin_zero: [0; 8],
        };

        if bind(
            socket_fd,
            &address as *const sockaddr_in as *const sockaddr,
            std::mem::size_of::<sockaddr_in>() as u32,
        ) < 0
        {
            eprintln!("Failed to bind socket to address");
            std::process::exit(1);
        }

        // Start listening at address
        if listen(socket_fd, 128) < 0 {
            eprintln!("Failed to listen on socket");
            std::process::exit(1);
        }

        println!("Server is listening on port {}", port);
        Ok(socket_fd)
    }
}

pub async fn heartbeat_check(heartbeat_fd: i32, mut heartbeat_rx: tokio::sync::mpsc::Receiver<()>) {
    unsafe {
        loop {
            tokio::time::sleep(HEARTBEAT_INTERVAL).await;
            let ping_msg = format!(
                "PING\nPlease respond with {}{}PONG{} if you are still active: \n",
                BOLD, CYAN, RESET
            );
            if write(
                heartbeat_fd,
                ping_msg.as_ptr() as *const c_void,
                ping_msg.len(),
            ) < 0
            {
                break;
            }

            if (tokio::time::timeout(TIMEOUT, heartbeat_rx.recv()).await).is_err() {
                close(heartbeat_fd);
                break;
            }
        }
    }
}

pub async fn connection_loop(
    heartbeat_tx: tokio::sync::mpsc::Sender<()>,
    client_fd: i32,
    database: Arc<Mutex<Database>>,
) {
    unsafe {
        let mut buffer: [u8; 1024] = [0; 1024];

        loop {
            let welcome_msg: String =
                format!("Available Commands:\n1) ADD <name> <phone> {}- Add a new friend{}\n2) GET <name> {}- Retrieve a friend's phone number{}\n3) DELETE <name> {}- Remove a friend from the database{}\nEXIT {}- Disconnect from the server{}\n",PINK,RESET,PINK,RESET,PINK,RESET,PINK,RESET);

            write(
                client_fd,
                welcome_msg.as_ptr() as *const c_void,
                welcome_msg.len(),
            );
            // Read the input from the client
            let bytes_read = read(client_fd, buffer.as_mut_ptr() as *mut c_void, buffer.len());
            if bytes_read <= 0 {
                break;
            }

            let mut input = str::from_utf8(&buffer[0..bytes_read as usize])
                .unwrap_or_default()
                .split_whitespace();

            let command = input.next();

            match command {
                Some("PONG") | Some("pong") => {
                    let _ = heartbeat_tx.send(()).await;
                }
                Some("ADD") | Some("add") | Some("Add") => {
                    if let (Some(name), Some(phone)) = (input.next(), input.next()) {
                        match database.lock().await.add_friend(name, phone) {
                            Ok(_) => {
                                let response: String =
                                    format!("Added {} with number {}\n", name, phone);
                                write(
                                    client_fd,
                                    response.as_ptr() as *const c_void,
                                    response.len(),
                                );
                            }
                            Err(e) => {
                                let response: String =
                                    format!("Error adding friend to the database:{:?}\n", e);
                                write(
                                    client_fd,
                                    response.as_ptr() as *const c_void,
                                    response.len(),
                                );
                            }
                        }
                    }
                }
                Some("GET") | Some("get") | Some("Get") => {
                    if let Some(name) = input.next() {
                        match database.lock().await.get_friend(name) {
                            Ok(Some(phone)) => {
                                let response: String = format!("{} : {}\n", name, phone);
                                write(
                                    client_fd,
                                    response.as_ptr() as *const c_void,
                                    response.len(),
                                );
                            }
                            Ok(None) => {
                                let response: String = String::from("Error friend not found\n");
                                write(
                                    client_fd,
                                    response.as_ptr() as *const c_void,
                                    response.len(),
                                );
                            }
                            Err(e) => {
                                let response: String = format!("Error retrieving friend:{:?}\n", e);
                                write(
                                    client_fd,
                                    response.as_ptr() as *const c_void,
                                    response.len(),
                                );
                            }
                        }
                    }
                }
                Some("DELETE") | Some("delete") | Some("Delete") => {
                    if let Some(name) = input.next() {
                        match database.lock().await.delete_friend(name) {
                            Ok(_) => {
                                let response: String =
                                    format!("{} has been removed from the database.\n", name);
                                write(
                                    client_fd,
                                    response.as_ptr() as *const c_void,
                                    response.len(),
                                );
                            }
                            Err(e) => {
                                let response: String = format!("Error removing friend:{:?}\n", e);
                                write(
                                    client_fd,
                                    response.as_ptr() as *const c_void,
                                    response.len(),
                                );
                            }
                        }
                    }
                }
                Some("EXIT") | Some("exit") => {
                    let goodbye_msg: &str = "Goodbye!\n";
                    write(
                        client_fd,
                        goodbye_msg.as_ptr() as *const c_void,
                        goodbye_msg.len(),
                    );
                    close(client_fd);
                    break;
                }
                _ => {
                    let error_msg: &str = "Invalid input. Please enter a valid command.\n";
                    write(
                        client_fd,
                        error_msg.as_ptr() as *const c_void,
                        error_msg.len(),
                    );
                }
            }
        }
    }
}

pub async fn handle_telnet_connection(
    client_fd: i32,
    database: Arc<Mutex<Database>>,
) -> Result<(), Box<dyn Error>> {
    unsafe {
        let welcome_msg: String = format!(
            "{}{}Welcome to the Telnet Friend Database!{}\n",
            CLEAR_SCREEN, BOLD, RESET
        );

        write(
            client_fd,
            welcome_msg.as_ptr() as *const c_void,
            welcome_msg.len(),
        );

        let (heartbeat_tx, heartbeat_rx) = tokio::sync::mpsc::channel::<()>(1);
        let connection_task = tokio::spawn(connection_loop(
            heartbeat_tx,
            client_fd,
            Arc::clone(&database),
        ));
        let heartbeat_task = tokio::spawn(heartbeat_check(client_fd, heartbeat_rx));

        tokio::select! {
            _ = connection_task => {
            },
            _ = heartbeat_task => {
            },
        }
    }
    Ok(())
}
