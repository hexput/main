//! Windows service executable for hexput
//!
//! This binary runs hexput as a native Windows service.
//! Build with: cargo build --release --features windows-service --bin hexput-service

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::ffi::OsString;
    use std::time::Duration;
    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };

    const SERVICE_NAME: &str = "hexput";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_arguments: Vec<OsString>) {
        if let Err(e) = run_service() {
            eprintln!("Service error: {}", e);
        }
    }

    fn run_service() -> Result<(), Box<dyn std::error::Error>> {
        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Interrogate => {
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

        // Tell Windows we're starting
        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        // Run the actual service logic
        // For now, this is a placeholder - in production, this would:
        // 1. Create a named pipe server
        // 2. Listen for incoming connections
        // 3. Process hexput scripts via RPC

        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async { run_hexput_service().await })?;

        // Tell Windows we're stopping
        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        Ok(())
    }

    async fn run_hexput_service() -> Result<(), Box<dyn std::error::Error>> {
        // Service logic here
        // This would typically:
        // 1. Set up named pipe listener on \\.\pipe\hexput
        // 2. Accept connections
        // 3. Process RPC requests
        // 4. Execute hexput scripts with proper sandboxing

        println!("Hexput service running...");

        // For now, just keep running until stopped
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    // Dispatch the service
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This binary is only available on Windows.");
    eprintln!("Build with: cargo build --release --features windows-service --bin hexput-service");
    std::process::exit(1);
}
