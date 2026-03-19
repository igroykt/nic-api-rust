use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

use crate::error::{DnsApiError, Result};

#[cfg(test)]
mod tests {
    use super::*;

    // ── NicService::from_xml ──────────────────────────────────────────────────

    #[test]
    fn nic_service_from_xml() {
        let xml = r#"<service admin="admin" domains-limit="10" domains-num="5" enable="true" has-primary="true" name="myservice" payer="payer" tariff="free" />"#;
        let svc = NicService::from_xml(xml).unwrap();
        assert_eq!(svc.admin, "admin");
        assert_eq!(svc.domains_limit, 10);
        assert_eq!(svc.domains_num, 5);
        assert!(svc.enable);
        assert!(svc.has_primary);
        assert_eq!(svc.name, "myservice");
        assert_eq!(svc.payer, "payer");
        assert_eq!(svc.tariff, "free");
        assert_eq!(svc.rr_limit, None);
        assert_eq!(svc.rr_num, None);
    }

    #[test]
    fn nic_service_from_xml_with_rr() {
        let xml = r#"<service admin="a" domains-limit="1" domains-num="1" enable="false" has-primary="false" name="s" payer="p" tariff="t" rr-limit="100" rr-num="10" />"#;
        let svc = NicService::from_xml(xml).unwrap();
        assert_eq!(svc.rr_limit, Some(100));
        assert_eq!(svc.rr_num, Some(10));
    }

    #[test]
    fn nic_service_from_xml_missing_element() {
        let result = NicService::from_xml("<other />");
        assert!(result.is_err());
    }

    // ── NicZone::from_xml ─────────────────────────────────────────────────────

    #[test]
    fn nic_zone_from_xml() {
        let xml = r#"<zone admin="admin" enable="true" has-changes="false" has-primary="true" id="12345" idn-name="example.com" name="example.com" payer="payer" service="myservice" />"#;
        let zone = NicZone::from_xml(xml).unwrap();
        assert_eq!(zone.admin, "admin");
        assert!(zone.enable);
        assert!(!zone.has_changes);
        assert!(zone.has_primary);
        assert_eq!(zone.id, 12345);
        assert_eq!(zone.idn_name, "example.com");
        assert_eq!(zone.name, "example.com");
        assert_eq!(zone.payer, "payer");
        assert_eq!(zone.service, "myservice");
    }

    #[test]
    fn nic_zone_from_xml_missing_element() {
        let result = NicZone::from_xml("<other />");
        assert!(result.is_err());
    }

    // ── NicZoneRevision::from_xml ─────────────────────────────────────────────

    #[test]
    fn nic_zone_revision_from_xml() {
        let xml = r#"<revision date="2023-01-01" ip="1.2.3.4" number="42" />"#;
        let rev = NicZoneRevision::from_xml(xml).unwrap();
        assert_eq!(rev.date, "2023-01-01");
        assert_eq!(rev.ip, "1.2.3.4");
        assert_eq!(rev.number, 42);
    }

    // ── ARecord ───────────────────────────────────────────────────────────────

    #[test]
    fn a_record_new() {
        let r = ARecord::new("@", "1.2.3.4");
        assert_eq!(r.name, "@");
        assert_eq!(r.a, "1.2.3.4");
        assert_eq!(r.id, None);
        assert_eq!(r.idn_name, None);
        assert_eq!(r.ttl, None);
    }

    #[test]
    fn a_record_with_ttl() {
        let r = ARecord::new("@", "1.2.3.4").with_ttl(3600).unwrap();
        assert_eq!(r.ttl, Some(3600));
    }

    #[test]
    fn a_record_with_ttl_zero_err() {
        let err = ARecord::new("@", "1.2.3.4").with_ttl(0).unwrap_err();
        assert!(matches!(err, DnsApiError::InvalidTtl));
    }

    #[test]
    fn a_record_to_xml_no_id_no_ttl() {
        let xml = ARecord::new("@", "1.2.3.4").to_xml().unwrap();
        assert!(xml.contains("<rr>"));
        assert!(!xml.contains("id="));
        assert!(xml.contains("<type>A</type>"));
        assert!(xml.contains("<a>1.2.3.4</a>"));
        assert!(!xml.contains("<ttl>"));
    }

    #[test]
    fn a_record_to_xml_with_id_and_ttl() {
        let mut r = ARecord::new("@", "1.2.3.4").with_ttl(3600).unwrap();
        r.id = Some(123);
        let xml = r.to_xml().unwrap();
        assert!(xml.contains(r#"id="123""#));
        assert!(xml.contains("<ttl>3600</ttl>"));
    }

    // ── AaaaRecord ────────────────────────────────────────────────────────────

    #[test]
    fn aaaa_record_new() {
        let r = AaaaRecord::new("@", "::1");
        assert_eq!(r.name, "@");
        assert_eq!(r.aaaa, "::1");
        assert_eq!(r.id, None);
        assert_eq!(r.idn_name, None);
    }

    #[test]
    fn aaaa_record_with_ttl_zero_err() {
        assert!(matches!(
            AaaaRecord::new("@", "::1").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn aaaa_record_to_xml() {
        let xml = AaaaRecord::new("@", "::1").to_xml().unwrap();
        assert!(xml.contains("<type>AAAA</type>"));
        assert!(xml.contains("<aaaa>::1</aaaa>"));
    }

    // ── CnameRecord ───────────────────────────────────────────────────────────

    #[test]
    fn cname_record_new() {
        let r = CnameRecord::new("www", "example.com.");
        assert_eq!(r.name, "www");
        assert_eq!(r.cname, "example.com.");
        assert_eq!(r.id, None);
    }

    #[test]
    fn cname_record_with_ttl_zero_err() {
        assert!(matches!(
            CnameRecord::new("www", "example.com.").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn cname_record_to_xml() {
        let xml = CnameRecord::new("www", "example.com.").to_xml().unwrap();
        assert!(xml.contains("<type>CNAME</type>"));
        assert!(xml.contains("<cname>example.com.</cname>"));
    }

    // ── MxRecord ──────────────────────────────────────────────────────────────

    #[test]
    fn mx_record_new() {
        let r = MxRecord::new("@", 10, "mail.example.com.");
        assert_eq!(r.preference, 10);
        assert_eq!(r.exchange, "mail.example.com.");
        assert_eq!(r.id, None);
    }

    #[test]
    fn mx_record_with_ttl_zero_err() {
        assert!(matches!(
            MxRecord::new("@", 10, "mail.example.com.").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn mx_record_to_xml() {
        let xml = MxRecord::new("@", 10, "mail.example.com.").to_xml().unwrap();
        assert!(xml.contains("<type>MX</type>"));
        assert!(xml.contains("<preference>10</preference>"));
        assert!(xml.contains("<exchange>mail.example.com.</exchange>"));
    }

    // ── NsRecord ──────────────────────────────────────────────────────────────

    #[test]
    fn ns_record_new() {
        let r = NsRecord::new("@", "ns1.example.com.");
        assert_eq!(r.name, "@");
        assert_eq!(r.ns, "ns1.example.com.");
        assert_eq!(r.id, None);
        assert_eq!(r.idn_name, None);
    }

    #[test]
    fn ns_record_to_xml() {
        let xml = NsRecord::new("@", "ns1.example.com.").to_xml().unwrap();
        assert!(xml.contains("<type>NS</type>"));
        assert!(xml.contains("<ns>ns1.example.com.</ns>"));
    }

    // ── TxtRecord ─────────────────────────────────────────────────────────────

    #[test]
    fn txt_record_new() {
        let r = TxtRecord::new("@", "v=spf1 -all");
        assert_eq!(r.name, "@");
        assert_eq!(r.txt, "v=spf1 -all");
        assert_eq!(r.id, None);
        assert_eq!(r.idn_name, None);
    }

    #[test]
    fn txt_record_with_ttl_zero_err() {
        assert!(matches!(
            TxtRecord::new("@", "v=spf1 -all").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn txt_record_to_xml_wraps_in_string() {
        let xml = TxtRecord::new("@", "v=spf1 -all").to_xml().unwrap();
        assert!(xml.contains("<type>TXT</type>"));
        assert!(xml.contains("<txt><string>v=spf1 -all</string></txt>"));
    }

    // ── SoaRecord ─────────────────────────────────────────────────────────────

    #[test]
    fn soa_record_new() {
        let r = SoaRecord::new("@", "ns1.example.com.", "admin.example.com.", 2023010101, 3600, 900, 604800, 300);
        assert_eq!(r.mname, "ns1.example.com.");
        assert_eq!(r.rname, "admin.example.com.");
        assert_eq!(r.serial, 2023010101);
        assert_eq!(r.id, None);
        assert_eq!(r.idn_name, None);
    }

    #[test]
    fn soa_record_to_xml() {
        let r = SoaRecord::new("@", "ns1.example.com.", "admin.example.com.", 2023010101, 3600, 900, 604800, 300);
        let xml = r.to_xml().unwrap();
        assert!(xml.contains("<type>SOA</type>"));
        assert!(xml.contains("<mname>ns1.example.com.</mname>"));
        assert!(xml.contains("<rname>admin.example.com.</rname>"));
        assert!(xml.contains("<serial>2023010101</serial>"));
    }

    // ── SrvRecord ─────────────────────────────────────────────────────────────

    #[test]
    fn srv_record_new() {
        let r = SrvRecord::new("_sip._tcp", 10, 20, 5060, "sip.example.com.");
        assert_eq!(r.priority, 10);
        assert_eq!(r.weight, 20);
        assert_eq!(r.port, 5060);
        assert_eq!(r.target, "sip.example.com.");
        assert_eq!(r.id, None);
        assert_eq!(r.idn_name, None);
    }

    #[test]
    fn srv_record_with_ttl_zero_err() {
        assert!(matches!(
            SrvRecord::new("_sip._tcp", 10, 20, 5060, "sip.example.com.").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn srv_record_to_xml() {
        let xml = SrvRecord::new("_sip._tcp", 10, 20, 5060, "sip.example.com.").to_xml().unwrap();
        assert!(xml.contains("<type>SRV</type>"));
        assert!(xml.contains("<priority>10</priority>"));
        assert!(xml.contains("<port>5060</port>"));
        assert!(xml.contains("<target>sip.example.com.</target>"));
    }

    // ── PtrRecord ─────────────────────────────────────────────────────────────

    #[test]
    fn ptr_record_new() {
        let r = PtrRecord::new("4.3.2.1.in-addr.arpa.", "example.com.");
        assert_eq!(r.ptr, "example.com.");
        assert_eq!(r.id, None);
    }

    #[test]
    fn ptr_record_with_ttl_zero_err() {
        assert!(matches!(
            PtrRecord::new("4.3.2.1.in-addr.arpa.", "example.com.").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn ptr_record_to_xml() {
        let xml = PtrRecord::new("4.3.2.1.in-addr.arpa.", "example.com.").to_xml().unwrap();
        assert!(xml.contains("<type>PTR</type>"));
        assert!(xml.contains("<ptr>example.com.</ptr>"));
    }

    // ── DnameRecord ───────────────────────────────────────────────────────────

    #[test]
    fn dname_record_new() {
        let r = DnameRecord::new("sub", "example.com.");
        assert_eq!(r.dname, "example.com.");
        assert_eq!(r.id, None);
    }

    #[test]
    fn dname_record_with_ttl_zero_err() {
        assert!(matches!(
            DnameRecord::new("sub", "example.com.").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn dname_record_to_xml() {
        let xml = DnameRecord::new("sub", "example.com.").to_xml().unwrap();
        assert!(xml.contains("<type>DNAME</type>"));
        assert!(xml.contains("<dname>example.com.</dname>"));
    }

    // ── HinfoRecord ───────────────────────────────────────────────────────────

    #[test]
    fn hinfo_record_new() {
        let r = HinfoRecord::new("@", "x86", "Linux");
        assert_eq!(r.hardware, "x86");
        assert_eq!(r.os, "Linux");
        assert_eq!(r.id, None);
    }

    #[test]
    fn hinfo_record_with_ttl_zero_err() {
        assert!(matches!(
            HinfoRecord::new("@", "x86", "Linux").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn hinfo_record_to_xml() {
        let xml = HinfoRecord::new("@", "x86", "Linux").to_xml().unwrap();
        assert!(xml.contains("<type>HINFO</type>"));
        assert!(xml.contains("<hardware>x86</hardware>"));
        assert!(xml.contains("<os>Linux</os>"));
    }

    // ── NaptrRecord ───────────────────────────────────────────────────────────

    #[test]
    fn naptr_record_new() {
        let r = NaptrRecord::new("@", 100, 10, "U", "E2U+sip");
        assert_eq!(r.order, 100);
        assert_eq!(r.preference, 10);
        assert_eq!(r.flags, "U");
        assert_eq!(r.service, "E2U+sip");
        assert_eq!(r.regexp, None);
        assert_eq!(r.replacement, None);
        assert_eq!(r.id, None);
    }

    #[test]
    fn naptr_record_with_ttl_zero_err() {
        assert!(matches!(
            NaptrRecord::new("@", 100, 10, "U", "E2U+sip").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn naptr_record_to_xml_no_optional() {
        let xml = NaptrRecord::new("@", 100, 10, "U", "E2U+sip").to_xml().unwrap();
        assert!(xml.contains("<type>NAPTR</type>"));
        assert!(xml.contains("<order>100</order>"));
        assert!(xml.contains("<flags>U</flags>"));
        assert!(!xml.contains("<regexp>"));
        assert!(!xml.contains("<replacement>"));
    }

    #[test]
    fn naptr_record_to_xml_with_optional() {
        let mut r = NaptrRecord::new("@", 100, 10, "U", "E2U+sip");
        r.regexp = Some("!^.*$!sip:info@example.com!".to_string());
        r.replacement = Some(".".to_string());
        let xml = r.to_xml().unwrap();
        assert!(xml.contains("<regexp>"));
        assert!(xml.contains("<replacement>.</replacement>"));
    }

    // ── RpRecord ──────────────────────────────────────────────────────────────

    #[test]
    fn rp_record_new() {
        let r = RpRecord::new("@", "admin.example.com.", "info.example.com.");
        assert_eq!(r.mbox, "admin.example.com.");
        assert_eq!(r.txt, "info.example.com.");
        assert_eq!(r.id, None);
    }

    #[test]
    fn rp_record_with_ttl_zero_err() {
        assert!(matches!(
            RpRecord::new("@", "admin.example.com.", "info.example.com.").with_ttl(0).unwrap_err(),
            DnsApiError::InvalidTtl
        ));
    }

    #[test]
    fn rp_record_to_xml() {
        let xml = RpRecord::new("@", "admin.example.com.", "info.example.com.").to_xml().unwrap();
        assert!(xml.contains("<type>RP</type>"));
        assert!(xml.contains("<mbox-dname>admin.example.com.</mbox-dname>"));
        assert!(xml.contains("<txt-dname>info.example.com.</txt-dname>"));
    }

    // ── DnsRecord enum ────────────────────────────────────────────────────────

    #[test]
    fn dns_record_id_none() {
        let r = DnsRecord::A(ARecord::new("@", "1.2.3.4"));
        assert_eq!(r.id(), None);
    }

    #[test]
    fn dns_record_id_some() {
        let mut a = ARecord::new("@", "1.2.3.4");
        a.id = Some(42);
        let r = DnsRecord::A(a);
        assert_eq!(r.id(), Some(42));
    }

    #[test]
    fn dns_record_name() {
        let r = DnsRecord::AAAA(AaaaRecord::new("mail", "::1"));
        assert_eq!(r.name(), "mail");
    }

    #[test]
    fn dns_record_to_xml_delegates() {
        let r = DnsRecord::TXT(TxtRecord::new("@", "hello"));
        let xml = r.to_xml().unwrap();
        assert!(xml.contains("<type>TXT</type>"));
    }

    #[test]
    fn dns_record_name_all_variants() {
        let records: Vec<DnsRecord> = vec![
            DnsRecord::A(ARecord::new("a", "1.2.3.4")),
            DnsRecord::AAAA(AaaaRecord::new("aaaa", "::1")),
            DnsRecord::CNAME(CnameRecord::new("cname", "x.")),
            DnsRecord::MX(MxRecord::new("mx", 10, "mail.")),
            DnsRecord::NS(NsRecord::new("ns", "ns1.")),
            DnsRecord::TXT(TxtRecord::new("txt", "v=spf1")),
            DnsRecord::SOA(SoaRecord::new("soa", "ns1.", "admin.", 1, 1, 1, 1, 1)),
            DnsRecord::SRV(SrvRecord::new("srv", 0, 0, 80, "target.")),
            DnsRecord::PTR(PtrRecord::new("ptr", "host.")),
            DnsRecord::DNAME(DnameRecord::new("dname", "x.")),
            DnsRecord::HINFO(HinfoRecord::new("hinfo", "x86", "Linux")),
            DnsRecord::NAPTR(NaptrRecord::new("naptr", 1, 1, "U", "sip")),
            DnsRecord::RP(RpRecord::new("rp", "mbox.", "txt.")),
        ];
        let names = ["a","aaaa","cname","mx","ns","txt","soa","srv","ptr","dname","hinfo","naptr","rp"];
        for (record, expected) in records.iter().zip(names.iter()) {
            assert_eq!(record.name(), *expected);
        }
    }
}

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
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
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
    pub ttl: Option<u32>,
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

impl TxtRecord {
    pub fn new(name: impl Into<String>, txt: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            txt: txt.into(),
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
        writer.write_event(Event::Text(BytesText::new("TXT")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("txt")))?;
        writer.write_event(Event::Start(BytesStart::new("string")))?;
        writer.write_event(Event::Text(BytesText::new(&self.txt)))?;
        writer.write_event(Event::End(BytesEnd::new("string")))?;
        writer.write_event(Event::End(BytesEnd::new("txt")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoaRecord {
    pub id: Option<u64>,
    pub name: String,
    pub idn_name: Option<String>,
    pub ttl: Option<u32>,
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

impl AaaaRecord {
    pub fn new(name: impl Into<String>, aaaa: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            aaaa: aaaa.into(),
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
        writer.write_event(Event::Text(BytesText::new("AAAA")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("aaaa")))?;
        writer.write_event(Event::Text(BytesText::new(&self.aaaa)))?;
        writer.write_event(Event::End(BytesEnd::new("aaaa")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl CnameRecord {
    pub fn new(name: impl Into<String>, cname: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            cname: cname.into(),
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
        writer.write_event(Event::Text(BytesText::new("CNAME")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("cname")))?;
        writer.write_event(Event::Text(BytesText::new(&self.cname)))?;
        writer.write_event(Event::End(BytesEnd::new("cname")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl MxRecord {
    pub fn new(name: impl Into<String>, preference: u16, exchange: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            preference,
            exchange: exchange.into(),
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
        writer.write_event(Event::Text(BytesText::new("MX")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("mx")))?;

        writer.write_event(Event::Start(BytesStart::new("preference")))?;
        writer.write_event(Event::Text(BytesText::new(&self.preference.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("preference")))?;

        writer.write_event(Event::Start(BytesStart::new("exchange")))?;
        writer.write_event(Event::Text(BytesText::new(&self.exchange)))?;
        writer.write_event(Event::End(BytesEnd::new("exchange")))?;

        writer.write_event(Event::End(BytesEnd::new("mx")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl NsRecord {
    pub fn new(name: impl Into<String>, ns: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            ns: ns.into(),
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
        writer.write_event(Event::Text(BytesText::new("NS")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("ns")))?;
        writer.write_event(Event::Text(BytesText::new(&self.ns)))?;
        writer.write_event(Event::End(BytesEnd::new("ns")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl SoaRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        mname: impl Into<String>,
        rname: impl Into<String>,
        serial: u32,
        refresh: u32,
        retry: u32,
        expire: u32,
        minimum: u32,
    ) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            serial,
            refresh,
            retry,
            expire,
            minimum,
            mname: mname.into(),
            rname: rname.into(),
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
        writer.write_event(Event::Text(BytesText::new("SOA")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("soa")))?;

        writer.write_event(Event::Start(BytesStart::new("mname")))?;
        writer.write_event(Event::Text(BytesText::new(&self.mname)))?;
        writer.write_event(Event::End(BytesEnd::new("mname")))?;

        writer.write_event(Event::Start(BytesStart::new("rname")))?;
        writer.write_event(Event::Text(BytesText::new(&self.rname)))?;
        writer.write_event(Event::End(BytesEnd::new("rname")))?;

        writer.write_event(Event::Start(BytesStart::new("serial")))?;
        writer.write_event(Event::Text(BytesText::new(&self.serial.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("serial")))?;

        writer.write_event(Event::Start(BytesStart::new("refresh")))?;
        writer.write_event(Event::Text(BytesText::new(&self.refresh.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("refresh")))?;

        writer.write_event(Event::Start(BytesStart::new("retry")))?;
        writer.write_event(Event::Text(BytesText::new(&self.retry.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("retry")))?;

        writer.write_event(Event::Start(BytesStart::new("expire")))?;
        writer.write_event(Event::Text(BytesText::new(&self.expire.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("expire")))?;

        writer.write_event(Event::Start(BytesStart::new("minimum")))?;
        writer.write_event(Event::Text(BytesText::new(&self.minimum.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("minimum")))?;

        writer.write_event(Event::End(BytesEnd::new("soa")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl SrvRecord {
    pub fn new(
        name: impl Into<String>,
        priority: u16,
        weight: u16,
        port: u16,
        target: impl Into<String>,
    ) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            priority,
            weight,
            port,
            target: target.into(),
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
        writer.write_event(Event::Text(BytesText::new("SRV")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("srv")))?;

        writer.write_event(Event::Start(BytesStart::new("priority")))?;
        writer.write_event(Event::Text(BytesText::new(&self.priority.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("priority")))?;

        writer.write_event(Event::Start(BytesStart::new("weight")))?;
        writer.write_event(Event::Text(BytesText::new(&self.weight.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("weight")))?;

        writer.write_event(Event::Start(BytesStart::new("port")))?;
        writer.write_event(Event::Text(BytesText::new(&self.port.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("port")))?;

        writer.write_event(Event::Start(BytesStart::new("target")))?;
        writer.write_event(Event::Text(BytesText::new(&self.target)))?;
        writer.write_event(Event::End(BytesEnd::new("target")))?;

        writer.write_event(Event::End(BytesEnd::new("srv")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl PtrRecord {
    pub fn new(name: impl Into<String>, ptr: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            ptr: ptr.into(),
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
        writer.write_event(Event::Text(BytesText::new("PTR")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("ptr")))?;
        writer.write_event(Event::Text(BytesText::new(&self.ptr)))?;
        writer.write_event(Event::End(BytesEnd::new("ptr")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl DnameRecord {
    pub fn new(name: impl Into<String>, dname: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            dname: dname.into(),
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
        writer.write_event(Event::Text(BytesText::new("DNAME")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("dname")))?;
        writer.write_event(Event::Text(BytesText::new(&self.dname)))?;
        writer.write_event(Event::End(BytesEnd::new("dname")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl HinfoRecord {
    pub fn new(name: impl Into<String>, hardware: impl Into<String>, os: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            hardware: hardware.into(),
            os: os.into(),
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
        writer.write_event(Event::Text(BytesText::new("HINFO")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("hinfo")))?;

        writer.write_event(Event::Start(BytesStart::new("hardware")))?;
        writer.write_event(Event::Text(BytesText::new(&self.hardware)))?;
        writer.write_event(Event::End(BytesEnd::new("hardware")))?;

        writer.write_event(Event::Start(BytesStart::new("os")))?;
        writer.write_event(Event::Text(BytesText::new(&self.os)))?;
        writer.write_event(Event::End(BytesEnd::new("os")))?;

        writer.write_event(Event::End(BytesEnd::new("hinfo")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl NaptrRecord {
    pub fn new(
        name: impl Into<String>,
        order: u16,
        preference: u16,
        flags: impl Into<String>,
        service: impl Into<String>,
    ) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            order,
            preference,
            flags: flags.into(),
            service: service.into(),
            regexp: None,
            replacement: None,
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
        writer.write_event(Event::Text(BytesText::new("NAPTR")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("naptr")))?;

        writer.write_event(Event::Start(BytesStart::new("order")))?;
        writer.write_event(Event::Text(BytesText::new(&self.order.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("order")))?;

        writer.write_event(Event::Start(BytesStart::new("preference")))?;
        writer.write_event(Event::Text(BytesText::new(&self.preference.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("preference")))?;

        writer.write_event(Event::Start(BytesStart::new("flags")))?;
        writer.write_event(Event::Text(BytesText::new(&self.flags)))?;
        writer.write_event(Event::End(BytesEnd::new("flags")))?;

        writer.write_event(Event::Start(BytesStart::new("service")))?;
        writer.write_event(Event::Text(BytesText::new(&self.service)))?;
        writer.write_event(Event::End(BytesEnd::new("service")))?;

        if let Some(regexp) = &self.regexp {
            writer.write_event(Event::Start(BytesStart::new("regexp")))?;
            writer.write_event(Event::Text(BytesText::new(regexp)))?;
            writer.write_event(Event::End(BytesEnd::new("regexp")))?;
        }

        if let Some(replacement) = &self.replacement {
            writer.write_event(Event::Start(BytesStart::new("replacement")))?;
            writer.write_event(Event::Text(BytesText::new(replacement)))?;
            writer.write_event(Event::End(BytesEnd::new("replacement")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("naptr")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}

impl RpRecord {
    pub fn new(name: impl Into<String>, mbox: impl Into<String>, txt: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            idn_name: None,
            ttl: None,
            mbox: mbox.into(),
            txt: txt.into(),
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
        writer.write_event(Event::Text(BytesText::new("RP")))?;
        writer.write_event(Event::End(BytesEnd::new("type")))?;

        writer.write_event(Event::Start(BytesStart::new("rp")))?;

        writer.write_event(Event::Start(BytesStart::new("mbox-dname")))?;
        writer.write_event(Event::Text(BytesText::new(&self.mbox)))?;
        writer.write_event(Event::End(BytesEnd::new("mbox-dname")))?;

        writer.write_event(Event::Start(BytesStart::new("txt-dname")))?;
        writer.write_event(Event::Text(BytesText::new(&self.txt)))?;
        writer.write_event(Event::End(BytesEnd::new("txt-dname")))?;

        writer.write_event(Event::End(BytesEnd::new("rp")))?;

        writer.write_event(Event::End(BytesEnd::new("rr")))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result).map_err(|e| DnsApiError::XmlError(e.to_string()))
    }
}
