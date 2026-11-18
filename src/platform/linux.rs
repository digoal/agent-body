use anyhow::Result;
use crate::fact::automation::AutomationTopology;
use crate::fact::process_net::{ListeningPortFact, ProcessNetTopology};
use super::{PlatformProbe, PlatformServiceFact};

/// Linux 探针：保留 `/proc` + systemd 微秒级零外依赖实现
pub struct LinuxProbe;

impl PlatformProbe for LinuxProbe {
    fn name(&self) -> &'static str {
        "linux"
    }

    fn listening_ports(&self) -> Result<Vec<ListeningPortFact>> {
        ProcessNetTopology::scan_listening_ports_procfs()
    }

    fn services(&self) -> Result<Vec<PlatformServiceFact>> {
        let units = AutomationTopology::scan_systemd_services()?;
        Ok(units
            .into_iter()
            .map(|u| PlatformServiceFact {
                name: u.service_name,
                command: u.exec_start,
                kind: "systemd".to_string(),
                source: u.unit_file_path,
            })
            .collect())
    }
}
