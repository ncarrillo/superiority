use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Read, Seek},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::Path,
};

use pcap_file::{
    DataLink,
    pcap::PcapReader,
    pcapng::{Block, PcapNgReader},
};

use crate::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TcpPacket {
    pub source: IpAddr,
    pub source_port: u16,
    pub destination: IpAddr,
    pub destination_port: u16,
    pub sequence: u32,
    pub payload: Vec<u8>,
    pub process_id: Option<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct TcpDirection {
    start_sequence: Option<u32>,
    data: Vec<u8>,
    pending: BTreeMap<u32, Vec<u8>>,
}

impl TcpDirection {
    pub fn add(&mut self, sequence: u32, payload: &[u8]) -> Result<bool> {
        if payload.is_empty() {
            return Ok(false);
        }
        let previous_len = self.data.len();
        if self.start_sequence.is_none() {
            self.start_sequence = Some(sequence);
            self.data.extend_from_slice(payload);
            self.flush_pending()?;
            return Ok(true);
        }

        let start = self.start_sequence.expect("the stream start was set");
        let end = start.wrapping_add(u32::try_from(self.data.len()).map_err(|_| {
            Error::Capture("TCP direction exceeded the 32-bit sequence space".to_owned())
        })?);
        if sequence.wrapping_sub(start) > 0x8000_0000 {
            let prefix = usize::try_from(start.wrapping_sub(sequence))
                .map_err(|_| Error::Capture("TCP prefix exceeds platform limits".to_owned()))?;
            if prefix <= payload.len() {
                let overlap = &payload[prefix..];
                let comparable = overlap.len().min(self.data.len());
                if self.data[..comparable] != overlap[..comparable] {
                    return Err(Error::Capture(format!(
                        "conflicting TCP segment before sequence {start}"
                    )));
                }
                self.data.splice(0..0, payload[..prefix].iter().copied());
                self.start_sequence = Some(sequence);
                self.flush_pending()?;
            } else {
                self.insert_pending(sequence, payload)?;
            }
            return Ok(self.data.len() != previous_len);
        }
        if sequence.wrapping_sub(end) < 0x8000_0000 && sequence != end {
            self.insert_pending(sequence, payload)?;
            return Ok(false);
        }

        let offset = usize::try_from(sequence.wrapping_sub(start)).map_err(|_| {
            Error::Capture("TCP sequence offset exceeds platform limits".to_owned())
        })?;
        if offset < self.data.len() {
            let comparable = payload.len().min(self.data.len() - offset);
            if self.data[offset..offset + comparable] != payload[..comparable] {
                return Err(Error::Capture(format!(
                    "conflicting TCP retransmission at sequence {sequence}"
                )));
            }
            if payload.len() > comparable {
                self.data.extend_from_slice(&payload[comparable..]);
            }
        } else {
            self.data.extend_from_slice(payload);
        }
        self.flush_pending()?;
        Ok(self.data.len() != previous_len)
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    fn insert_pending(&mut self, sequence: u32, payload: &[u8]) -> Result<()> {
        if let Some(previous) = self.pending.get(&sequence)
            && previous != payload
        {
            return Err(Error::Capture(format!(
                "conflicting out-of-order TCP segment at sequence {sequence}"
            )));
        }
        self.pending.insert(sequence, payload.to_vec());
        Ok(())
    }

    fn flush_pending(&mut self) -> Result<()> {
        loop {
            let Some(start) = self.start_sequence else {
                return Ok(());
            };
            let end = start.wrapping_add(u32::try_from(self.data.len()).map_err(|_| {
                Error::Capture("TCP direction exceeded the 32-bit sequence space".to_owned())
            })?);
            let candidate = self
                .pending
                .keys()
                .copied()
                .find(|sequence| sequence.wrapping_sub(end) > 0x8000_0000 || *sequence == end);
            let Some(sequence) = candidate else {
                return Ok(());
            };
            let payload = self
                .pending
                .remove(&sequence)
                .expect("the pending segment was selected");
            self.add(sequence, &payload)?;
        }
    }
}

pub fn read_capture(path: &Path) -> Result<Vec<TcpPacket>> {
    let mut file = BufReader::new(File::open(path)?);
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic)?;
    let mut file = file.into_inner();
    file.rewind()?;
    if magic == [0x0a, 0x0d, 0x0d, 0x0a] {
        read_pcapng(file)
    } else {
        read_pcap(file)
    }
}

fn read_pcap(file: File) -> Result<Vec<TcpPacket>> {
    let mut reader = PcapReader::new(BufReader::new(file))?;
    let link = reader.header().datalink;
    let mut packets = Vec::new();
    while let Some(packet) = reader.next_packet() {
        let packet = packet?;
        if let Some(packet) = parse_link_packet(link, &packet.data)? {
            packets.push(packet);
        }
    }
    Ok(packets)
}

fn read_pcapng(file: File) -> Result<Vec<TcpPacket>> {
    let mut reader = PcapNgReader::new(BufReader::new(file))?;
    let mut packets = Vec::new();
    let mut interfaces = Vec::new();
    while let Some(block) = reader.next_block() {
        let block = block?;
        match block {
            Block::SectionHeader(_) => interfaces.clear(),
            Block::InterfaceDescription(interface) => interfaces.push(interface.linktype),
            Block::EnhancedPacket(packet) => {
                let link = interfaces
                    .get(usize::try_from(packet.interface_id).map_err(|_| {
                        Error::Capture("pcapng interface id exceeds platform limits".to_owned())
                    })?)
                    .copied()
                    .ok_or_else(|| {
                        Error::Capture("pcapng packet references a missing interface".to_owned())
                    })?;
                if let Some(packet) = parse_link_packet(link, &packet.data)? {
                    packets.push(packet);
                }
            }
            _ => {}
        }
    }
    Ok(packets)
}

pub fn parse_link_packet(link: DataLink, data: &[u8]) -> Result<Option<TcpPacket>> {
    let (link, data, process_id) = if link == DataLink::PKTAP {
        let pktap = parse_pktap(data)?;
        (pktap.link, pktap.payload, Some(pktap.process_id))
    } else {
        (link, data, None)
    };
    let ip = match link {
        DataLink::RAW | DataLink::IPV4 | DataLink::IPV6 | DataLink::Unknown(12) => data,
        DataLink::NULL | DataLink::LOOP => data.get(4..).ok_or_else(|| {
            Error::Capture("loopback packet is shorter than its family header".to_owned())
        })?,
        DataLink::ETHERNET => ethernet_payload(data)?,
        _ => {
            return Err(Error::Capture(format!(
                "unsupported packet data-link type {link:?}"
            )));
        }
    };
    Ok(parse_ip_packet(ip, process_id))
}

struct PktapPacket<'a> {
    link: DataLink,
    payload: &'a [u8],
    process_id: i32,
}

fn parse_pktap(data: &[u8]) -> Result<PktapPacket<'_>> {
    if data.len() < 40 {
        return Err(Error::Capture("PKTAP header is truncated".to_owned()));
    }
    let flags = read_u32_le(data, 36)?;
    let version_two = flags & 0x0008_0000 != 0;
    let (length, dlt, process_id) = if version_two {
        (
            usize::from(data[0]),
            u32::from(read_u16_le(data, 6)?),
            read_i32_le(data, 28)?,
        )
    } else {
        (
            usize::try_from(read_u32_le(data, 0)?).map_err(|_| {
                Error::Capture("PKTAP header length exceeds platform limits".to_owned())
            })?,
            read_u32_le(data, 8)?,
            read_i32_le(data, 52)?,
        )
    };
    if length > data.len() || length < 40 {
        return Err(Error::Capture(format!(
            "PKTAP header declares invalid length {length}"
        )));
    }
    Ok(PktapPacket {
        link: DataLink::from(dlt),
        payload: &data[length..],
        process_id,
    })
}

fn ethernet_payload(data: &[u8]) -> Result<&[u8]> {
    if data.len() < 14 {
        return Err(Error::Capture("Ethernet frame is truncated".to_owned()));
    }
    let mut offset = 14;
    let mut ether_type = u16::from_be_bytes([data[12], data[13]]);
    while matches!(ether_type, 0x8100 | 0x88a8) {
        let header = data
            .get(offset..offset + 4)
            .ok_or_else(|| Error::Capture("VLAN header is truncated".to_owned()))?;
        ether_type = u16::from_be_bytes([header[2], header[3]]);
        offset += 4;
    }
    data.get(offset..)
        .ok_or_else(|| Error::Capture("Ethernet payload is truncated".to_owned()))
}

fn parse_ip_packet(data: &[u8], process_id: Option<i32>) -> Option<TcpPacket> {
    let version = data.first().map(|byte| byte >> 4)?;
    let (source, destination, tcp_offset, end) = match version {
        4 => {
            if data.len() < 20 {
                return None;
            }
            let header = usize::from(data[0] & 0x0f) * 4;
            let total = usize::from(u16::from_be_bytes([data[2], data[3]]));
            if header < 20 || total > data.len() || data[9] != 6 {
                return None;
            }
            let fragment = u16::from_be_bytes([data[6], data[7]]);
            if fragment & 0x1fff != 0 {
                return None;
            }
            (
                IpAddr::V4(Ipv4Addr::new(data[12], data[13], data[14], data[15])),
                IpAddr::V4(Ipv4Addr::new(data[16], data[17], data[18], data[19])),
                header,
                total,
            )
        }
        6 => {
            if data.len() < 40 || data[6] != 6 {
                return None;
            }
            let payload = usize::from(u16::from_be_bytes([data[4], data[5]]));
            let end = (40 + payload).min(data.len());
            let source = Ipv6Addr::from(<[u8; 16]>::try_from(&data[8..24]).expect("fixed slice"));
            let destination =
                Ipv6Addr::from(<[u8; 16]>::try_from(&data[24..40]).expect("fixed slice"));
            (IpAddr::V6(source), IpAddr::V6(destination), 40, end)
        }
        _ => return None,
    };
    if end < tcp_offset + 20 {
        return None;
    }
    let source_port = u16::from_be_bytes([data[tcp_offset], data[tcp_offset + 1]]);
    let destination_port = u16::from_be_bytes([data[tcp_offset + 2], data[tcp_offset + 3]]);
    if source_port != 1119 && destination_port != 1119 {
        return None;
    }
    let sequence = u32::from_be_bytes(
        data[tcp_offset + 4..tcp_offset + 8]
            .try_into()
            .expect("fixed slice"),
    );
    let tcp_header = usize::from(data[tcp_offset + 12] >> 4) * 4;
    if tcp_header < 20 || tcp_offset + tcp_header > end {
        return None;
    }
    Some(TcpPacket {
        source,
        source_port,
        destination,
        destination_port,
        sequence,
        payload: data[tcp_offset + tcp_header..end].to_vec(),
        process_id,
    })
}

fn read_u16_le(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| Error::Capture("packet header is truncated".to_owned()))?;
    Ok(u16::from_le_bytes(bytes.try_into().expect("fixed slice")))
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| Error::Capture("packet header is truncated".to_owned()))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("fixed slice")))
}

fn read_i32_le(data: &[u8], offset: usize) -> Result<i32> {
    Ok(i32::from_le_bytes(
        data.get(offset..offset + 4)
            .ok_or_else(|| Error::Capture("packet header is truncated".to_owned()))?
            .try_into()
            .expect("fixed slice"),
    ))
}

#[cfg(target_os = "macos")]
pub mod live;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ipv4_tcp_payload() {
        let mut packet = vec![0x45, 0, 0, 43, 0, 0, 0, 0, 64, 6, 0, 0];
        packet.extend_from_slice(&[127, 0, 0, 1, 10, 0, 0, 1]);
        packet.extend_from_slice(&1119_u16.to_be_bytes());
        packet.extend_from_slice(&50000_u16.to_be_bytes());
        packet.extend_from_slice(&123_u32.to_be_bytes());
        packet.extend_from_slice(&0_u32.to_be_bytes());
        packet.extend_from_slice(&[0x50, 0x18, 0, 0, 0, 0, 0, 0]);
        packet.extend_from_slice(b"abc");
        let parsed = parse_link_packet(DataLink::RAW, &packet).unwrap().unwrap();
        assert_eq!(parsed.source_port, 1119);
        assert_eq!(parsed.destination_port, 50000);
        assert_eq!(parsed.sequence, 123);
        assert_eq!(parsed.payload, b"abc");
    }

    #[test]
    fn parses_macos_raw_link_type() {
        let mut packet = vec![0x45, 0, 0, 43, 0, 0, 0, 0, 64, 6, 0, 0];
        packet.extend_from_slice(&[127, 0, 0, 1, 10, 0, 0, 1]);
        packet.extend_from_slice(&1119_u16.to_be_bytes());
        packet.extend_from_slice(&50000_u16.to_be_bytes());
        packet.extend_from_slice(&123_u32.to_be_bytes());
        packet.extend_from_slice(&0_u32.to_be_bytes());
        packet.extend_from_slice(&[0x50, 0x18, 0, 0, 0, 0, 0, 0]);
        packet.extend_from_slice(b"abc");
        let parsed = parse_link_packet(DataLink::Unknown(12), &packet)
            .unwrap()
            .unwrap();
        assert_eq!(parsed.payload, b"abc");
    }

    #[test]
    fn reassembles_out_of_order_segments() {
        let mut direction = TcpDirection::default();
        direction.add(100, b"abc").unwrap();
        direction.add(106, b"ghi").unwrap();
        assert_eq!(direction.bytes(), b"abc");
        direction.add(103, b"def").unwrap();
        assert_eq!(direction.bytes(), b"abcdefghi");
        direction.add(100, b"abc").unwrap();
        assert_eq!(direction.bytes(), b"abcdefghi");
    }

    #[test]
    fn prepends_a_segment_that_overlaps_the_first_observed_segment() {
        let mut direction = TcpDirection::default();
        direction.add(103, b"def").unwrap();
        direction.add(100, b"abcdef").unwrap();
        assert_eq!(direction.bytes(), b"abcdef");
    }
}
