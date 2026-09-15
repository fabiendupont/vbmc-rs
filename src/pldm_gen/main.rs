//! Generate minimal, valid PLDM firmware-update packages (`.fwpkg`) for driving
//! the simulate-mode UpdateService with a real client (nvfwupd/RMS).
//!
//! A PLDM firmware package is a documented binary layout: a package header
//! (UUID + format revision + timestamp + version), one or more firmware-device
//! ID records (each with descriptors and an applicable-component bitmap), a
//! component-image-information area, then the component payloads. Real packages
//! carry signed vendor firmware; a simulator only needs bytes that *parse*, so
//! the payloads here are deterministic filler.
//!
//! The output is a format-revision-1 package (UUID `F0188…`), which is the
//! simplest valid variant: no downstream-device area, no reference manifest, no
//! per-component opaque data, and no payload checksum. nvfwupd reads but does
//! not validate the header checksum, so it is written as zero.
//!
//! Verify a generated package with the client's own tooling:
//!   nvfwupd show_pkg_content -p <out.fwpkg>
//!   nvfwupd unpack -p <out.fwpkg> -o <dir>

use std::fs;
use std::process::ExitCode;

use clap::Parser;

/// Format-revision-1 package header UUID (must match one of nvfwupd's known
/// UUIDs or the package is rejected as "Not a valid PLDM package").
const PLDM_V1_UUID: [u8; 16] = [
    0xF0, 0x18, 0x87, 0x8C, 0xCB, 0x7D, 0x49, 0x43, 0x98, 0x00, 0xA0, 0x2F, 0x05, 0x9A, 0xCA, 0x02,
];

/// NVIDIA's IANA Private Enterprise Number (5703), used for the initial
/// descriptor by default so the package looks like an NVIDIA one.
const NVIDIA_IANA: u32 = 5703;

/// ASCII version-string type per the PLDM firmware update spec.
const VERSION_STRING_ASCII: u8 = 1;

/// Descriptor type 0x0001 = "IANA Enterprise ID" (little-endian, 4 bytes).
const DESCRIPTOR_TYPE_IANA: u16 = 0x0001;

#[derive(Parser)]
#[command(
    name = "vbmc-rs-pldm-gen",
    about = "Generate a minimal valid PLDM firmware package for simulate-mode testing"
)]
struct Args {
    /// Output package path (e.g. bluefield-sim.fwpkg).
    #[arg(short, long)]
    out: String,

    /// Package header version string.
    #[arg(long, default_value = "sim-1.0")]
    pkg_version: String,

    /// Component as NAME=VERSION. Repeat for multiple components; each becomes a
    /// component image with a deterministic placeholder payload.
    #[arg(short, long = "component", value_name = "NAME=VERSION", required = true)]
    components: Vec<String>,

    /// IANA Enterprise ID for the initial device descriptor.
    #[arg(long, default_value_t = NVIDIA_IANA)]
    iana: u32,

    /// Placeholder payload size (bytes) per component image.
    #[arg(long, default_value_t = 256)]
    payload_size: u32,
}

struct Component {
    name: String,
    version: String,
}

fn parse_components(raw: &[String]) -> Result<Vec<Component>, String> {
    if raw.len() > 64 {
        return Err(format!(
            "at most 64 components are supported (got {})",
            raw.len()
        ));
    }
    raw.iter()
        .map(|spec| {
            let (name, version) = spec
                .split_once('=')
                .ok_or_else(|| format!("component '{spec}' must be NAME=VERSION"))?;
            if name.is_empty() || version.is_empty() {
                return Err(format!("component '{spec}' must be NAME=VERSION"));
            }
            Ok(Component {
                name: name.to_string(),
                version: version.to_string(),
            })
        })
        .collect()
}

/// Smallest supported component-bitmap bit length (multiple of 8, 8..=64) that
/// covers `count` components.
fn bitmap_bit_length(count: usize) -> u16 {
    match count {
        0..=8 => 8,
        9..=16 => 16,
        17..=32 => 32,
        _ => 64,
    }
}

trait ByteSink {
    fn u8(&mut self, v: u8);
    fn u16(&mut self, v: u16);
    fn u32(&mut self, v: u32);
    /// ASCII string preceded by a 1-byte length (as PLDM version strings are
    /// encoded: a separate type byte is written by the caller).
    fn lstr(&mut self, s: &str);
}

impl ByteSink for Vec<u8> {
    fn u8(&mut self, v: u8) {
        self.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn lstr(&mut self, s: &str) {
        self.u8(s.len() as u8);
        self.extend_from_slice(s.as_bytes());
    }
}

/// Build the package header + records + component-image-info block, up to and
/// including the (zeroed) header checksum. Component `location_offset`s are set
/// to `offsets[i]`; on the first pass pass zeros to measure the block length,
/// then rebuild with real offsets (the block length is invariant to the offset
/// values since they are fixed-width fields).
fn build_header_block(args: &Args, components: &[Component], offsets: &[u32]) -> Vec<u8> {
    let bit_len = bitmap_bit_length(components.len());
    let bitmap_bytes = (bit_len / 8) as usize;

    let mut b: Vec<u8> = Vec::new();

    // --- Package header information ---
    b.extend_from_slice(&PLDM_V1_UUID);
    b.u8(1); // format revision
    b.u16(0); // header size (read but unused by the parser)
    b.extend_from_slice(&[0u8; 13]); // release timestamp (zeroed)
    b.u16(bit_len); // component bitmap bit length
    b.u8(VERSION_STRING_ASCII);
    b.lstr(&args.pkg_version);

    // --- Firmware device ID records: one record applicable to all components ---
    b.u8(1); // record count

    // applicable-components bitmap: low `count` bits set.
    let mut applicable: u64 = 0;
    for i in 0..components.len() {
        applicable |= 1u64 << i;
    }
    let applicable_le = applicable.to_le_bytes();

    b.u16(0); // record length (read but unused)
    b.u8(1); // descriptor count (single initial descriptor)
    b.u32(0); // device update option flags
    b.u8(VERSION_STRING_ASCII); // component image set version string type
    b.u8(args.pkg_version.len() as u8); // ...length
    b.u16(0); // firmware package data length
    b.extend_from_slice(&applicable_le[..bitmap_bytes]);
    b.extend_from_slice(args.pkg_version.as_bytes()); // component image set version string
    // Initial descriptor: IANA Enterprise ID (little-endian, 4 bytes).
    b.u16(DESCRIPTOR_TYPE_IANA);
    b.u16(4);
    b.u32(args.iana);
    // (no firmware package data: length above is 0)

    // --- Component image information ---
    b.u16(components.len() as u16);
    for (i, comp) in components.iter().enumerate() {
        b.u16(0x000A); // classification (Firmware); read but unused
        b.u16((i + 1) as u16); // component identifier
        b.u32(0); // comparison stamp
        b.u16(0); // options
        b.u16(0); // requested activation method
        b.u32(offsets[i]); // location offset (filled on the real pass)
        b.u32(args.payload_size); // size
        b.u8(VERSION_STRING_ASCII);
        b.lstr(&comp.version);
    }

    b.u32(0); // package header checksum (read but not validated)
    b
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let components = parse_components(&args.components)?;

    // Pass 1: measure the header block with placeholder offsets.
    let placeholder = vec![0u32; components.len()];
    let header_len = build_header_block(&args, &components, &placeholder).len() as u32;

    // Payloads are laid out contiguously right after the header block.
    let mut offsets = Vec::with_capacity(components.len());
    let mut cursor = header_len;
    for _ in &components {
        offsets.push(cursor);
        cursor = cursor
            .checked_add(args.payload_size)
            .ok_or_else(|| "package size overflow".to_string())?;
    }

    // Pass 2: rebuild with real offsets, then append deterministic payloads.
    let mut pkg = build_header_block(&args, &components, &offsets);
    debug_assert_eq!(pkg.len() as u32, header_len);
    for (i, _) in components.iter().enumerate() {
        // Fill each payload with a per-component byte so unpack output differs.
        pkg.extend(std::iter::repeat_n((i + 1) as u8, args.payload_size as usize));
    }

    fs::write(&args.out, &pkg).map_err(|e| format!("cannot write {}: {e}", args.out))?;
    println!(
        "wrote {} ({} bytes): {} component(s), IANA {}, header {} bytes",
        args.out,
        pkg.len(),
        components.len(),
        args.iana,
        header_len
    );
    for comp in &components {
        println!("  - {} = {}", comp.name, comp.version);
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_for(components: &[&str]) -> Args {
        Args {
            out: "/dev/null".into(),
            pkg_version: "BF-24.10-1".into(),
            components: components.iter().map(|s| s.to_string()).collect(),
            iana: NVIDIA_IANA,
            payload_size: 256,
        }
    }

    #[test]
    fn parses_name_version_pairs() {
        let parsed = parse_components(&["BMC=1.0".into(), "NIC=2.3.4".into()]).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "BMC");
        assert_eq!(parsed[1].version, "2.3.4");
    }

    #[test]
    fn rejects_malformed_component() {
        assert!(parse_components(&["noequals".into()]).is_err());
        assert!(parse_components(&["=1.0".into()]).is_err());
        assert!(parse_components(&["name=".into()]).is_err());
    }

    #[test]
    fn bitmap_length_covers_component_count() {
        assert_eq!(bitmap_bit_length(1), 8);
        assert_eq!(bitmap_bit_length(8), 8);
        assert_eq!(bitmap_bit_length(9), 16);
        assert_eq!(bitmap_bit_length(32), 32);
        assert_eq!(bitmap_bit_length(33), 64);
    }

    #[test]
    fn header_block_length_is_offset_invariant() {
        // Pass 1 (placeholder offsets) and pass 2 (real offsets) must produce
        // the same block length, or the computed payload offsets are wrong.
        let args = args_for(&["A=1", "B=2", "C=3"]);
        let components = parse_components(&args.components).unwrap();
        let zeros = vec![0u32; components.len()];
        let reals = vec![1000u32, 2000, 3000];
        assert_eq!(
            build_header_block(&args, &components, &zeros).len(),
            build_header_block(&args, &components, &reals).len()
        );
    }

    #[test]
    fn header_starts_with_valid_uuid() {
        let args = args_for(&["A=1"]);
        let components = parse_components(&args.components).unwrap();
        let block = build_header_block(&args, &components, &[0]);
        assert_eq!(&block[..16], &PLDM_V1_UUID);
        assert_eq!(block[16], 1); // format revision
    }
}
