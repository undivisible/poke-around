use poke_around::bridge_state::*;
use std::time::Instant;
use tokio::time::sleep;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let _ = poke_around::config::ensure_private_config_dir();
    let start = Instant::now();
    let mut handles = vec![];
    for _ in 0..50 {
        handles.push(tokio::spawn(async {
            for _ in 0..10 {
                let _ = read_state();
            }
        }));
    }
    let sleep_handle = tokio::spawn(async {
        let s = Instant::now();
        sleep(Duration::from_millis(50)).await;
        s.elapsed()
    });
    for h in handles {
        let _ = h.await;
    }
    let sleep_elapsed = sleep_handle.await.unwrap();
    println!("Time for 50ms sleep task: {:?}", sleep_elapsed);
    println!("Total time: {:?}", start.elapsed());
}
