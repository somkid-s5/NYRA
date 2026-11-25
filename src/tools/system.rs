use crate::{
    CpuData, DiskData, DiskVolume, MeasurementAvailability, MemoryData, RunError, Sha256Data,
    Sha256Target, SystemInfoData,
};
use sha2::{Digest, Sha256};
use std::time::Duration;
use sysinfo::{CpuRefreshKind, Disks, System};
use tokio::{
    io::AsyncReadExt,
    time::{Instant, sleep, timeout_at},
};

pub(crate) fn system_info() -> Result<SystemInfoData, RunError> {
    Ok(SystemInfoData {
        os: std::env::consts::OS.to_owned(),
        architecture: System::cpu_arch(),
        hostname: System::host_name().ok(),
        os_name: System::name().ok(),
        os_version: System::os_version().ok(),
        kernel_version: System::kernel_version().ok(),
        uptime_seconds: System::uptime().ok(),
    })
}

pub(crate) async fn cpu() -> Result<CpuData, RunError> {
    let mut system = System::new().map_err(|_| RunError::SystemInfoUnavailable)?;
    system.refresh_cpu_list(CpuRefreshKind::everything());
    sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    system.refresh_cpu_usage();
    let cpus = system.cpus();
    let Some(first) = cpus.first() else {
        return Err(RunError::SystemInfoUnavailable);
    };
    let average_usage = cpus.iter().map(|cpu| cpu.usage()).sum::<f32>() / cpus.len() as f32;
    let usage = cpus
        .iter()
        .take(1024)
        .map(|cpu| cpu.usage())
        .collect::<Vec<_>>();
    Ok(CpuData {
        brand: (!first.brand().trim().is_empty()).then(|| first.brand().trim().to_owned()),
        logical_cores: cpus.len(),
        physical_cores: System::physical_core_count().ok(),
        frequency_mhz: (first.frequency() > 0).then_some(first.frequency()),
        average_usage_percent: Some(average_usage),
        per_core_usage_percent: usage,
    })
}

pub(crate) fn memory() -> Result<MemoryData, RunError> {
    let mut system = System::new().map_err(|_| RunError::SystemInfoUnavailable)?;
    system.refresh_memory();
    let total_bytes = system.total_memory();
    let available_bytes = system.available_memory();
    Ok(MemoryData {
        total_bytes,
        used_bytes: total_bytes.saturating_sub(available_bytes),
        available_bytes,
    })
}

pub(crate) fn disk() -> Result<DiskData, RunError> {
    let disks = Disks::new_with_refreshed_list().map_err(|_| RunError::SystemInfoUnavailable)?;
    let volumes = disks
        .iter()
        .take(256)
        .map(|disk| DiskVolume {
            name: disk.name().to_string_lossy().into_owned(),
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
            file_system: disk.file_system().to_string_lossy().into_owned(),
            total_bytes: disk.total_space(),
            available_bytes: disk.available_space(),
        })
        .collect::<Vec<_>>();
    Ok(DiskData {
        availability: if volumes.is_empty() {
            MeasurementAvailability::Unavailable
        } else {
            MeasurementAvailability::Available
        },
        volumes,
    })
}

pub(crate) async fn sha256(target: Sha256Target) -> Result<Sha256Data, RunError> {
    let deadline = Instant::now() + Duration::from_secs(target.timeout_seconds);
    let path_metadata = timeout_at(deadline, tokio::fs::metadata(&target.path))
        .await
        .map_err(|_| RunError::DeadlineExceeded)?
        .map_err(|_| RunError::FileOpenFailed)?;
    if !path_metadata.is_file() {
        return Err(RunError::FileOpenFailed);
    }
    let mut options = tokio::fs::OpenOptions::new();
    options.read(true);
    configure_nonblocking_file_open(&mut options);
    let mut file = timeout_at(deadline, options.open(&target.path))
        .await
        .map_err(|_| RunError::DeadlineExceeded)?
        .map_err(|_| RunError::FileOpenFailed)?;
    let metadata = timeout_at(deadline, file.metadata())
        .await
        .map_err(|_| RunError::DeadlineExceeded)?
        .map_err(|_| RunError::FileOpenFailed)?;
    if !metadata.is_file() {
        return Err(RunError::FileOpenFailed);
    }

    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size_bytes = 0_u64;
    loop {
        let bytes_read = timeout_at(deadline, file.read(&mut buffer))
            .await
            .map_err(|_| RunError::DeadlineExceeded)?
            .map_err(|_| RunError::FileReadFailed)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        size_bytes = size_bytes.saturating_add(bytes_read as u64);
    }

    Ok(Sha256Data {
        path: target.path.to_string_lossy().into_owned(),
        size_bytes,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

#[cfg(unix)]
fn configure_nonblocking_file_open(options: &mut tokio::fs::OpenOptions) {
    options.custom_flags(libc::O_NONBLOCK);
}

#[cfg(not(unix))]
fn configure_nonblocking_file_open(_options: &mut tokio::fs::OpenOptions) {}
