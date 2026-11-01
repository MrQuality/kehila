#[cfg(target_os = "linux")]
#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let (_stop, receiver) = tokio::sync::oneshot::channel();
    kehila_local_tls_probe::serve_owner(std::env::args().skip(1).collect(), receiver).await?;
    println!("Disposable owner runtime stopped and joined.");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The disposable owner backend requires Linux.");
    std::process::exit(1);
}
