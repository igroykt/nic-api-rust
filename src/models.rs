use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

use crate::error::{DnsApiError, Result};

/// Represents a NIC.RU DNS service
#[derive(Debug, Clone, PartialEq)]
pub struct NicService {
    pub admin: String,
    pub domains_limit: u32,
    pub domains_num: u32,
    pub enable: bool,
    pub has_primary: bool,
    pub name: String,
    pub payer: String,
    pub tariff: String,
    pub rr_limit: Option<u32>,
    pub rr_num: Option<u32>,
}

impl NicService {
    pub fn from_xml(xml: &str) -> Result<Self> {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut service = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"service" => {
                    let mut admin = String::new();
                    let mut domains_limit = 0u32;
                    let mut domains_num = 0u32;
                    let mut enable = false;
                    let mut has_primary = false;
                    let mut name = String::new();
                    let mut payer = String::new();
                    let mut tariff = String::new();
                    let mut rr_limit = None;
                    let mut rr_num = None;

                    for attr in e.attributes() {
                        let attr = attr?;
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap();
                        let value = attr.unescape_value().unwrap();

                        match key {
                            "admin" => admin = value.to_string(),
                            "domains-limit" => domains_limit = value.parse().unwrap_or(0),
                            "domains-num" => domains_num = value.parse().unwrap_or(0),
                            "enable" => enable = value == "true",
                            "has-primary" => has_primary = value == "true",
                            "name" => name = value.to_string(),
                            "payer" => payer = value.to_string(),
                            "tariff" => tariff = value.to_string(),
                            "rr-limit" => rr_limit = value.parse().ok(),
                            "rr-num" => rr_num = value.parse().ok(),
                            _ => {}
                        }
                    }

                    service = Some(NicService {
                        admin,
                        domains_limit,
                        domains_num,
                        enable,
                        has_primary,
                        name,
                        payer,
                        tariff,
                        rr_limit,
                        rr_num,
                    });
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        service.ok_or_else(|| DnsApiError::XmlError("No service element found".to_string()))
    }
}

/// Represents a DNS zone in NIC.RU
#[derive(Debug, Clone, PartialEq)]
pub struct NicZone {
    pub admin: String,
    pub enable: bool,
    pub has_changes: bool,
    pub has_primary: bool,
    pub id: u64,
    pub idn_name: String,
    pub name: String,
    pub payer: String,
    pub service: String,
}

impl NicZone {
    pub fn from_xml(xml: &str) -> Result<Self> {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut zone = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"zone" => {
                    let mut admin = String::new();
                    let mut enable = false;
                    let mut has_changes = false;
                    let mut has_primary = false;
                    let mut id = 0u64;
                    let mut idn_name = String::new();
                    let mut name = String::new();
                    let mut payer = String::new();
                    let mut service = String::new();

                    for attr in e.attributes() {
                        let attr = attr?;
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap();
                        let value = attr.unescape_value().unwrap();

                        match key {
                            "admin" => admin = value.to_string(),
                            "enable" => enable = value == "true",
                            "has-changes" => has_changes = value == "true",
                            "has-primary" => has_primary = value == "true",
                            "id" => id = value.parse().unwrap_or(0),
                            "idn-name" => idn_name = value.to_string(),
                            "name" => name = value.to_string(),
                            "payer" => payer = value.to_string(),
                            "service" => service = value.to_string(),
                            _ => {}
                        }
                    }

                    zone = Some(NicZone {
                        admin,
                        enable,
                        has_changes,
                        has_primary,
                        id,
                        idn_name,
                        name,
                        payer,
                        service,
                    });
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        zone.ok_or_else(|| DnsApiError::XmlError("No zone element found".to_string()))
    }
}

/// Represents a zone revision
#[derive(Debug, Clone, PartialEq)]
pub struct NicZoneRevision {
    pub date: String,
    pub ip: String,
    pub number: u32,
}

impl NicZoneRevision {
    pub fn from_xml(xml: &str) -> Result<Self> {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut revision = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"revision" => {
                    let mut date = String::new();
                    let mut ip = String::new();
                    let mut number = 0u32;

                    for attr in e.attributes() {
                        let attr = attr?;
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap();
                        let value = attr.unescape_value().unwrap();

                        match key {
                            "date" => date = value.to_string(),
                            "ip" => ip = value.to_string(),
                            "number" => number = value.parse().unwrap_or(0),
                            _ => {}
                        }
                    }

                    revision = Some(NicZoneRevision { date, ip, number });
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DnsApiError::XmlError(e.to_string())),
                _ => {}
            }
            buf.clear();
        }

        revision.ok_or_else(|| DnsApiError::XmlError("No revision element found".to_string()))
    }
}

/// DNS record types supported by NIC.RU API
#[derive(Debug, Clone, PartialEq)]
pub enum DnsRecord {
    A(ARecord),
    AAAA(AaaaRecord),
    CNAME(CnameRecord),
    MX(MxRecord),
    NS(NsRecord),
    TXT(TxtRecord),
    SOA(SoaRecord),
    SRV(SrvRecord),
    PTR(PtrRecord),
    DNAME(DnameRecord),
    HINFO(HinfoRecord),
    NAPTR(NaptrRecord),
    RP(RpRecord),
}

impl DnsRecord {
    pub fn id(&self) -> Option<u64> {
        match self {
            DnsRecord::A(r) => r.id,
            DnsRecord::AAAA(r) => r.id,
            DnsRecord::CNAME(r) => r.id,
            DnsRecord::MX(r) => r.id,
            DnsRecord::NS(r) => r.id,
            DnsRecord::TXT(r) => r.id,
            DnsRecord::SOA(r) => r.id,
            DnsRecord::SRV(r) => r.id,
            DnsRecord::PTR(r) => r.id,
            DnsRecord::DNAME(r) => r.id,
            DnsRecord::HINFO(r) => r.id,
            DnsRecord::NAPTR(r) => r.id,
            DnsRecord::RP(r) => r.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            DnsRecord::A(r) => &r.name,
            DnsRecord::AAAA(r) => &r.name,
            DnsRecord::CNAME(r) => &r.name,
            DnsRecord::MX(r) => &r.name,
            DnsRecord::NS(r) => &r.name,
            DnsRecord::TXT(r) => &r.name,
            DnsRecord::SOA(r) => &r.name,
            DnsRecord::SRV(r) => &r.name,
            DnsRecord::PTR(r) => &r.name,
            DnsRecord::DNAME(r) => &r.name,
            DnsRecord::HINFO(r) => &r.name,
            DnsRecord::NAPTR(r) => &r.name,
            DnsRecord::RP(r) => &r.name,
        }
    }

    pub fn to_xml(&self) -> Result<String> {
        match self {
            DnsRecord::A(r) => r.to_xml(),
            DnsRecord::AAAA(r) => r.to_xml(),
            DnsRecord::CNAME(r) => r.to_xml(),
            DnsRecord::MX(r) => r.to_xml(),
            DnsRecord::NS(r) => r.to_xml(),
            DnsRecord::TXT(r) => r.to_xml(),
            DnsRecord::SOA(r) => r.to_xml(),
            DnsRecord::SRV(r) => r.to_xml(),
            DnsRecord::PTR(r) => r.to_xml(),
            DnsRecord::DNAME(r) => r.to_xml(),
            DnsRecord::HINFO(r) => r.to_xml(),
            DnsRecord::NAPTR(r) => r.to_xml(),
            DnsRecord::RP(r) => r.to_xml(),
        }
    }
}

/// A record - maps a hostname to an IPv4 address
#[derive(Debug, Clone, PartialEq)]
pub struct ARecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub a: String, // IPv4 address
}

impl ARecord {
    pub fn new(name: impl Into<String>, a: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            a: a.into(),
        }
    }

    pub fn with_ttl(mut self, ttl: u32) -> Result<Self> {
        if ttl == 0 {
            return Err(DnsApiError::InvalidTtl);
        }
        self.ttl = Some(ttl);
        Ok(self)
    }

    pub fn to_xml(&self) -> Result<String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        
        let mut rr = BytesStart::new("rr");
        if let Some(id) = self.id {
            rr.push_attribute(("id", id.to_string().as_str()));
        }
        writer.write_event(Event::Start(rr))?;

        writer.write_event(Event::Start(BytesStart::new("name")))?;
        writer.write_event(Event::Text(BytesText::new(&self.name)))?;
        writer.write_event(Event::End(BytesEnd::new("name")))?;

        if let Some(ttl) = self.ttl {
            writer.write_event(Event::Start(BytesStart::new("ttl")))?;
            writer.write_event(Event::Text(BytesText::new(&ttl.to_string())))?;
            writer.write_event(Event::End(BytesEnd::new("ttl")))?;
        }

        writer.write_event(Event::Start(BytesStart::new("type")))?;
        writer.write_event(Event::Text(BytesText::new("A")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("a")))?;
        writer.write_event(Event::Text(BytesText::new(&self.a)))?;
        writer.write_event(Event::End(BytesEnd::new("a")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        Ok(String::from_utf8(result).unwrap())
    }
}

// Placeholder implementations for other record types
// These will be expanded with full XML serialization/deserialization

#[derive(Debug, Clone, PartialEq)]
pub struct AaaaRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub aaaa: String, // IPv6 address
}

#[derive(Debug, Clone, PartialEq)]
pub struct CnameRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub cname: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MxRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub preference: u16,
    pub exchange: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NsRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ns: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TxtRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub txt: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoaRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub serial: u32,
    pub refresh: u32,
    pub retry: u32,
    pub expire: u32,
    pub minimum: u32,
    pub mname: String,
    pub rname: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SrvRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub priority: u16,
    pub weight: u16,
    pub port: u16,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PtrRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub ptr: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DnameRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub dname: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HinfoRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub hardware: String,
    pub os: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NaptrRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub order: u16,
    pub preference: u16,
    pub flags: String,
    pub service: String,
    pub regexp: Option<String>,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
    pub mbox: String,
    pub txt: String,
}

// Stub implementations for to_xml methods (will be expanded)
macro_rules! impl_to_xml_stub {
    ($record_type:ty, $type_name:expr) => {
        impl $record_type {
            pub fn to_xml(&self) -> Result<String> {
                Ok(format!("<rr><type>{}</type></rr>", $type_name))
            }
        }
    };
}

impl_to_xml_stub!(AaaaRecord, "AAAA");
impl_to_xml_stub!(CnameRecord, "CNAME");
impl_to_xml_stub!(MxRecord, "MX");
impl_to_xml_stub!(NsRecord, "NS");
impl_to_xml_stub!(TxtRecord, "TXT");
impl_to_xml_stub!(SoaRecord, "SOA");
impl_to_xml_stub!(SrvRecord, "SRV");
impl_to_xml_stub!(PtrRecord, "PTR");
impl_to_xml_stub!(DnameRecord, "DNAME");
impl_to_xml_stub!(HinfoRecord, "HINFO");
impl_to_xml_stub!(NaptrRecord, "NAPTR");
impl_to_xml_stub!(RpRecord, "RP");
