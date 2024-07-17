mod auth;
mod db;
mod fs;
mod http;
mod service;

// the outermost caller should definitely have a loop that periodically calls
// Status for each service to ensure that the threads haven't stopped, and then
// gracefully stop the server after logging whatever the error was

#[tokio::main]
async fn main() {
    panic!("oh no")
}
