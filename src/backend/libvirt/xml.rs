use quick_xml::Reader;
use quick_xml::events::Event;

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

    let mut disk_idx = 0u32;
    let mut nic_idx = 0u32;

    let mut ps = ParseState {
        in_vcpu: false,
        in_memory: false,
        memory_unit: String::from("KiB"),
        in_devices: false,
        current_disk: None,
        current_nic: None,
        current_hostdev: None,
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                handle_open_tag(&name, e, &mut ps);
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                handle_open_tag(&name, e, &mut ps);
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if ps.in_vcpu {
                    info.vcpu_count = text.trim().parse().unwrap_or(0);
                    ps.in_vcpu = false;
                }
                if ps.in_memory {
                    let raw_val: u64 = text.trim().parse().unwrap_or(0);
                    info.memory_bytes = match ps.memory_unit.as_str() {
                        "KiB" | "k" => raw_val * 1024,
                        "MiB" | "M" => raw_val * 1024 * 1024,
                        "GiB" | "G" => raw_val * 1024 * 1024 * 1024,
                        "bytes" | "b" => raw_val,
                        _ => raw_val * 1024,
                    };
                    ps.in_memory = false;
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "vcpu" => ps.in_vcpu = false,
                    "memory" => ps.in_memory = false,
                    "devices" => ps.in_devices = false,
                    "disk" if ps.current_disk.is_some() => {
                        if let Some(builder) = ps.current_disk.take() {
                            let protocol = match builder.bus.as_deref() {
                                Some("virtio") => bt::DiskProtocol::Virtio,
                                Some("sata") | Some("ide") => bt::DiskProtocol::Sata,
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
                    "interface" if ps.current_nic.is_some() => {
                        if let Some(builder) = ps.current_nic.take() {
                            info.nics.push(bt::NicInfo {
                                id: format!("NIC{nic_idx}"),
                                mac_address: builder.mac,
                                tap: builder.target_dev,
                                speed_mbps: 25000,
                            });
                            nic_idx += 1;
                        }
                    }
                    "hostdev" if ps.current_hostdev.is_some() => {
                        if let Some(builder) = ps.current_hostdev.take()
                            && builder.hostdev_type == "pci"
                        {
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

struct ParseState {
    in_vcpu: bool,
    in_memory: bool,
    memory_unit: String,
    in_devices: bool,
    current_disk: Option<DiskBuilder>,
    current_nic: Option<NicBuilder>,
    current_hostdev: Option<HostdevBuilder>,
}

fn handle_open_tag(name: &str, e: &quick_xml::events::BytesStart<'_>, ps: &mut ParseState) {
    match name {
        "vcpu" => ps.in_vcpu = true,
        "memory" => {
            ps.in_memory = true;
            for attr in e.attributes().flatten() {
                if attr.key.as_ref() == b"unit" {
                    ps.memory_unit = String::from_utf8_lossy(&attr.value).to_string();
                }
            }
        }
        "devices" => ps.in_devices = true,
        "disk" if ps.in_devices => {
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
            ps.current_disk = Some(builder);
        }
        "source" if ps.current_disk.is_some() => {
            if let Some(disk) = &mut ps.current_disk {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"file" || attr.key.as_ref() == b"dev" {
                        disk.source = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        "target" if ps.current_disk.is_some() => {
            if let Some(disk) = &mut ps.current_disk {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"bus" {
                        disk.bus = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                    if attr.key.as_ref() == b"dev" {
                        disk.target_dev = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        "readonly" if ps.current_disk.is_some() => {
            if let Some(disk) = &mut ps.current_disk {
                disk.readonly = true;
            }
        }
        "interface" if ps.in_devices => {
            ps.current_nic = Some(NicBuilder::default());
        }
        "mac" if ps.current_nic.is_some() => {
            if let Some(nic) = &mut ps.current_nic {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"address" {
                        nic.mac = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        "target" if ps.current_nic.is_some() => {
            if let Some(nic) = &mut ps.current_nic {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"dev" {
                        nic.target_dev = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        "model" if ps.current_nic.is_some() => {
            if let Some(nic) = &mut ps.current_nic {
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"type" {
                        nic.model = Some(String::from_utf8_lossy(&attr.value).to_string());
                    }
                }
            }
        }
        "hostdev" if ps.in_devices => {
            let mut builder = HostdevBuilder::default();
            for attr in e.attributes().flatten() {
                if attr.key.as_ref() == b"type" {
                    builder.hostdev_type = String::from_utf8_lossy(&attr.value).to_string();
                }
            }
            ps.current_hostdev = Some(builder);
        }
        "address" if ps.current_hostdev.is_some() => {
            if let Some(hd) = &mut ps.current_hostdev {
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
    target_dev: Option<String>,
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

    #[test]
    fn test_parse_memory_kib() {
        let xml = r#"
        <domain type='kvm'>
          <vcpu>1</vcpu>
          <memory unit='KiB'>1048576</memory>
          <devices></devices>
        </domain>
        "#;
        let info = parse_domain_xml(xml);
        assert_eq!(info.memory_bytes, 1048576 * 1024); // 1 GiB
    }

    #[test]
    fn test_parse_memory_gib() {
        let xml = r#"
        <domain type='kvm'>
          <vcpu>1</vcpu>
          <memory unit='GiB'>2</memory>
          <devices></devices>
        </domain>
        "#;
        let info = parse_domain_xml(xml);
        assert_eq!(info.memory_bytes, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_parse_readonly_disk() {
        let xml = r#"
        <domain type='kvm'>
          <vcpu>1</vcpu>
          <memory unit='MiB'>512</memory>
          <devices>
            <disk type='file' device='cdrom'>
              <source file='/tmp/boot.iso'/>
              <target dev='hda' bus='ide'/>
              <readonly/>
            </disk>
          </devices>
        </domain>
        "#;
        let info = parse_domain_xml(xml);
        assert_eq!(info.disks.len(), 1);
        assert!(info.disks[0].readonly);
        assert_eq!(info.disks[0].protocol, bt::DiskProtocol::Sata); // ide maps to SATA
    }

    #[test]
    fn test_parse_sata_disk() {
        let xml = r#"
        <domain type='kvm'>
          <vcpu>1</vcpu>
          <memory unit='MiB'>512</memory>
          <devices>
            <disk type='file' device='disk'>
              <source file='/tmp/disk.qcow2'/>
              <target dev='sda' bus='sata'/>
            </disk>
          </devices>
        </domain>
        "#;
        let info = parse_domain_xml(xml);
        assert_eq!(info.disks[0].protocol, bt::DiskProtocol::Sata);
    }

    #[test]
    fn test_parse_multiple_disks_and_nics() {
        let xml = r#"
        <domain type='kvm'>
          <vcpu>2</vcpu>
          <memory unit='MiB'>1024</memory>
          <devices>
            <disk type='file' device='disk'>
              <source file='/tmp/disk1.qcow2'/>
              <target dev='vda' bus='virtio'/>
            </disk>
            <disk type='file' device='disk'>
              <source file='/tmp/disk2.qcow2'/>
              <target dev='vdb' bus='virtio'/>
            </disk>
            <interface type='network'>
              <mac address='52:54:00:00:00:01'/>
            </interface>
            <interface type='bridge'>
              <mac address='52:54:00:00:00:02'/>
            </interface>
          </devices>
        </domain>
        "#;
        let info = parse_domain_xml(xml);
        assert_eq!(info.disks.len(), 2);
        assert_eq!(info.disks[0].id, "vda");
        assert_eq!(info.disks[1].id, "vdb");
        assert_eq!(info.nics.len(), 2);
        assert_eq!(info.nics[0].id, "NIC0");
        assert_eq!(info.nics[1].id, "NIC1");
        assert_eq!(
            info.nics[0].mac_address.as_deref(),
            Some("52:54:00:00:00:01")
        );
        assert_eq!(
            info.nics[1].mac_address.as_deref(),
            Some("52:54:00:00:00:02")
        );
    }

    #[test]
    fn test_parse_pci_hostdev() {
        let xml = r#"
        <domain type='kvm'>
          <vcpu>1</vcpu>
          <memory unit='MiB'>512</memory>
          <devices>
            <hostdev mode='subsystem' type='pci' managed='yes'>
              <source>
                <address domain='0x0000' bus='0x03' slot='0x00' function='0x0'/>
              </source>
            </hostdev>
          </devices>
        </domain>
        "#;
        let info = parse_domain_xml(xml);
        assert_eq!(info.pci_devices.len(), 1);
        assert_eq!(info.pci_devices[0].bdf, "0000:03:00.0");
        assert!(info.pci_devices[0].is_passthrough);
    }

    #[test]
    fn test_parse_empty_domain() {
        let xml = r#"<domain type='kvm'></domain>"#;
        let info = parse_domain_xml(xml);
        assert_eq!(info.vcpu_count, 0);
        assert_eq!(info.memory_bytes, 0);
        assert!(info.disks.is_empty());
        assert!(info.nics.is_empty());
        assert!(info.pci_devices.is_empty());
    }

    #[test]
    fn test_parse_hex_values() {
        assert_eq!(parse_hex("0x0000"), 0);
        assert_eq!(parse_hex("0x03"), 3);
        assert_eq!(parse_hex("0xff"), 255);
        assert_eq!(parse_hex("10"), 16);
        assert_eq!(parse_hex(""), 0);
    }
}
