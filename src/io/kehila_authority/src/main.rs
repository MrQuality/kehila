#[cfg(not(target_os = "linux"))]
compile_error!("The product authority requires Linux; Windows is a client platform");
#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    let attempt = match kehila_authority::prepare_startup_attempt() {
        Ok(value) => value,
        Err(_) => return std::process::ExitCode::FAILURE,
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(value) => value,
        Err(_) => return std::process::ExitCode::FAILURE,
    };
    runtime.block_on(run(attempt))
}

#[cfg(target_os = "linux")]
async fn run(attempt: kehila_authority::StartupAttempt) -> std::process::ExitCode {
    use tokio::sync::oneshot;
    let mut terminate =
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(_) => return std::process::ExitCode::FAILURE,
        };
    let (stop, receiver) = oneshot::channel();
    let server = kehila_authority::serve(std::env::args().skip(1).collect(), attempt, receiver);
    tokio::pin!(server);
    let result = tokio::select! {
        result=&mut server=>result,
        signal=tokio::signal::ctrl_c()=>{
            let _=stop.send(());
            let result=server.await;
            if signal.is_err(){return std::process::ExitCode::FAILURE}
            result
        },
        _=terminate.recv()=>{let _=stop.send(());server.await},
    };
    match result {
        Ok(_) => std::process::ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("Authority startup or owned shutdown failed");
            std::process::ExitCode::FAILURE
        }
    }
}
