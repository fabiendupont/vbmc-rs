use quick_xml::events::Event;
use quick_xml::Reader;

use crate::backend::types as bt;

/// Parsed representation of a libvirt domain XML.
#[derive(Debug, Default)]
pub struct DomainInfo {
    pub vcpu_count: u32,
    pub memory_bytes: u64,
    pub disks: Vec<bt::DiskInfo>,
    pub nics: Vec<bt::NicInfo>,
    pub pci_devices: Vec<bt::PciDeviceInfo>,
}

/// Parse a libvirt domain XML string to extract VM hardware information.
pub fn parse_domain_xml(xml: &str) -> DomainInfo {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut info = DomainInfo::default();
    let mut buf = Vec::new();

    let mut in_vcpu = false;
    let mut in_memory = false;
    let mut memory_unit = String::from("KiB");
    let mut disk_idx = 0u32;
    let mut nic_idx = 0u32;

    let mut current_disk: Option<DiskBuilder> = None;
    let mut current_nic: Option<NicBuilder> = None;
    let mut current_hostdev: Option<HostdevBuilder> = None;
    let mut in_devices = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                handle_open_tag(
                    &name,
                    e,
                    false,
                    &mut in_vcpu,
                    &mut in_memory,
                    &mut memory_unit,
                    &mut in_devices,
                    &mut current_disk,
                    &mut current_nic,
                    &mut current_hostdev,
                );
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                handle_open_tag(
                    &name,
                    e,
                    true,
                    &mut in_vcpu,
                    &mut in_memory,
                    &mut memory_unit,
                    &mut in_devices,
                    &mut current_disk,
                    &mut current_nic,
                    &mut current_hostdev,
                );
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_vcpu {
                    info.vcpu_count = text.trim().parse().unwrap_or(0);
                    in_vcpu = false;
                }
                if in_memory {
                    let raw_val: u64 = text.trim().parse().unwrap_or(0);
                    info.memory_bytes = match memory_unit.as_str() {
                        "KiB" | "k" => raw_val * 1024,
                        "MiB" | "M" => raw_val * 1024 * 1024,
                        "GiB" | "G" => raw_val * 1024 * 1024 * 1024,
                        "bytes" | "b" => raw_val,
                        _ => raw_val * 1024,
                    };
                    in_memory = false;
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "vcpu" => in_vcpu = false,
                    "memory" => in_memory = false,
                    "devices" => in_devices = false,
                    "disk" if current_disk.is_some() => {
                        if let Some(builder) = current_disk.take() {
                            let protocol = match builder.bus.as_deref() {
                                Some("virtio") => bt::DiskProtocol::Virtio,
                                Some("sata") | Some("ide") => bt::DiskProtocol::SATA,
                                _ => bt::DiskProtocol::Unknown,
                            };
                            let id = builder
                                .target_dev
                                .unwrap_or_else(|| format!("disk{disk_idx}"));
                            info.disks.push(bt::DiskInfo {
                                id,
                                path: builder.source,
                                capacity_bytes: None,
                                readonly: builder.readonly,
                                protocol,
                                media_type: bt::DiskMediaType::Virtual,
                            });
                            disk_idx += 1;
                        }
                    }
                    "interface" if current_nic.is_some() => {
                        if let Some(builder) = current_nic.take() {
                            info.nics.push(bt::NicInfo {
                                id: format!("NIC{nic_idx}"),
                                mac_address: builder.mac,
                                tap: None,
                                speed_mbps: 25000,
                            });
                            nic_idx += 1;
                        }
                    }
                    "hostdev" if current_hostdev.is_some() => {
                        if let Some(builder) = current_hostdev.take() {
                            if builder.hostdev_type == "pci" {
                                let bdf = format!(
                                    "{:04x}:{:02x}:{:02x}.{:x}",
                                    builder.domain, builder.bus, builder.slot, builder.function
                                );
                                info.pci_devices.push(bt::PciDeviceInfo {
                                    bdf,
                                    vendor_id: None,
                                    device_id: None,
                                    class_code: None,
                                    device_name: None,
                                    is_passthrough: true,
                                    functions: vec![bt::PciFunctionInfo {
                                        function_id: builder.function as u8,
                                        class_code: None,
                                        device_id: None,
                                        vendor_id: None,
                                    }],
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    info
}

fn handle_open_tag(
    name: &str,
    e: &quick_xml::events::BytesStart<'_>,
    _is_empty: bool,
    in_vcpu: &mut bool,
    in_memory: &mut bool,
    memory_unit: &mut String,
    in_devices: &mut bool,
    current_disk: &mut Option<DiskBuilder>,
    current_nic: &mut Option<NicBuilder>,
    current_hostdev: &mut Option<HostdevBuilder>,
) {
    match name {
        "vcpu" => *in_vcpu = true,
        "memory" => {
            *in_memory = true;
            for attr in e.attributes().flatten() {
                if attr.key.as_ref() == b"unit" {
                    *memory_unit = String::from_utf8_lossy(&attr.value).to_string();
                }
            }
        }
        "devices" => *in_devices = true,
        "disk" if *in_devices => {
            let mut builder = DiskBuilder::default();
            for attr in e.attributes().flatten() {
                match attr.key.as_ref() {
                    b"type" => {
                        builder.disk_type = String::from_utf8_lossy(&attr.value).to_string();
                    }
                    b"device" => {
                        builder.device = String::from_utf8_lossy(&attr.value).to_string();
                    }
                    _ => {}
                }
            }
            *current_disk = Some(builder);
        }
        "source" if current_disk.is_some() => {
            if let Some(disk) = current_disk {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"file" || attr.key.as_ref() == b"dev" {
                        disk.source = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        "target" if current_disk.is_some() => {
            if let Some(disk) = current_disk {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"bus" {
                        disk.bus = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                    if attr.key.as_ref() == b"dev" {
                        disk.target_dev =
                            Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        "readonly" if current_disk.is_some() => {
            if let Some(disk) = current_disk {
                disk.readonly = true;
            }
        }
        "interface" if *in_devices => {
            *current_nic = Some(NicBuilder::default());
        }
        "mac" if current_nic.is_some() => {
            if let Some(nic) = current_nic {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"address" {
                        nic.mac = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        "model" if current_nic.is_some() => {
            if let Some(nic) = current_nic {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"type" {
                        nic.model = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        "hostdev" if *in_devices => {
            let mut builder = HostdevBuilder::default();
            for attr in e.attributes().flatten() {
                if attr.key.as_ref() == b"type" {
                    builder.hostdev_type = String::from_utf8_lossy(&attr.value).to_string();
                }
            }
            *current_hostdev = Some(builder);
        }
        "address" if current_hostdev.is_some() => {
            if let Some(hd) = current_hostdev {
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"domain" => {
                            hd.domain = parse_hex(&String::from_utf8_lossy(&attr.value));
                        }
                        b"bus" => {
                            hd.bus = parse_hex(&String::from_utf8_lossy(&attr.value));
                        }
                        b"slot" => {
                            hd.slot = parse_hex(&String::from_utf8_lossy(&attr.value));
                        }
                        b"function" => {
                            hd.function = parse_hex(&String::from_utf8_lossy(&attr.value));
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
}

#[derive(Debug, Default)]
struct DiskBuilder {
    disk_type: String,
    device: String,
    source: Option<String>,
    bus: Option<String>,
    target_dev: Option<String>,
    readonly: bool,
}

#[derive(Debug, Default)]
struct NicBuilder {
    mac: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Default)]
struct HostdevBuilder {
    hostdev_type: String,
    domain: u32,
    bus: u32,
    slot: u32,
    function: u32,
}

fn parse_hex(s: &str) -> u32 {
    let s = s.strip_prefix("0x").unwrap_or(s);
    u32::from_str_radix(s, 16).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_domain() {
        let xml = r#"
        <domain type='kvm'>
          <vcpu>4</vcpu>
          <memory unit='MiB'>8192</memory>
          <devices>
            <disk type='file' device='disk'>
              <source file='/var/lib/libvirt/images/test.qcow2'/>
              <target dev='vda' bus='virtio'/>
            </disk>
            <interface type='network'>
              <mac address='52:54:00:12:34:56'/>
              <model type='virtio'/>
            </interface>
          </devices>
        </domain>
        "#;

        let info = parse_domain_xml(xml);
        assert_eq!(info.vcpu_count, 4);
        assert_eq!(info.memory_bytes, 8192 * 1024 * 1024);
        assert_eq!(info.disks.len(), 1);
        assert_eq!(info.disks[0].id, "vda");
        assert_eq!(
            info.disks[0].path.as_deref(),
            Some("/var/lib/libvirt/images/test.qcow2")
        );
        assert_eq!(info.nics.len(), 1);
        assert_eq!(
            info.nics[0].mac_address.as_deref(),
            Some("52:54:00:12:34:56")
        );
    }
}
