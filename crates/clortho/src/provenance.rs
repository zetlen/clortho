//! Who is asking: the requesting process and its ancestors.

use clortho_core::proto::ProcInfo;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

const MAX_DEPTH: usize = 12;
const MAX_CMDLINE: usize = 100;

/// The process `pid` followed by its parents, nearest first. Stops before
/// pid 1. Empty if the process is already gone.
pub fn chain(pid: u32) -> Vec<ProcInfo> {
    let Ok(mut sys) = System::new() else {
        return Vec::new();
    };
    let kind = ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always);
    let mut out = Vec::new();
    let mut cur = Pid::from_u32(pid);
    while out.len() < MAX_DEPTH && cur.as_u32() > 1 {
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[cur]), false, kind);
        let Some(p) = sys.process(cur) else { break };
        let mut cmdline = p
            .cmd()
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
            .replace(['\n', '\r'], " ");
        if cmdline.chars().count() > MAX_CMDLINE {
            cmdline = cmdline.chars().take(MAX_CMDLINE).collect::<String>() + "…";
        }
        out.push(ProcInfo {
            pid: cur.as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            cmdline,
        });
        match p.parent() {
            Some(parent) if parent != cur => cur = parent,
            _ => break,
        }
    }
    out
}
