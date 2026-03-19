//! # nic-api-rust Basic Usage Example
//!
//! This example demonstrates a comprehensive usage of the `nic_api_rust` library,
//! including authentication, listing services/zones/records, adding and deleting
//! records, zone lifecycle management, import/export, rollback, TTL management,
//! zone revisions, AXFR settings, master servers, and committing changes.
//!
//! ## Setup
//!
//! Set the following environment variables before running:
//!
//! ```sh
//! export NIC_APP_LOGIN="your_app_login"
//! export NIC_APP_PASSWORD="your_app_password"
//! export NIC_USERNAME="your_nic_username"
//! export NIC_PASSWORD="your_nic_password"
//! ```
//!
//! Then run:
//!
//! ```sh
//! cargo run --example basic_usage
//! ```
//!
//! > **Warning:** Sections marked "MODIFIES LIVE DNS" will create and delete real
//! > DNS records on your account. Review them carefully before running.

use nic_api_rust::{
    AaaaRecord, ARecord, DnsApi, DnsApiError, DnsRecord, MxRecord, TxtRecord,
};
use std::env;

#[tokio::main]
async fn main() {
    // -------------------------------------------------------------------------
    // 1. AUTHENTICATION
    // -------------------------------------------------------------------------
    // Read OAuth2 application credentials and user credentials from the
    // environment. The app credentials (login/password) are issued by NIC.RU
    // when you register an OAuth2 application. The username/password are your
    // regular NIC.RU account credentials.
    let app_login = env::var("NIC_APP_LOGIN")
        .expect("NIC_APP_LOGIN environment variable must be set");
    let app_password = env::var("NIC_APP_PASSWORD")
        .expect("NIC_APP_PASSWORD environment variable must be set");
    let username = env::var("NIC_USERNAME")
        .expect("NIC_USERNAME environment variable must be set");
    let password = env::var("NIC_PASSWORD")
        .expect("NIC_PASSWORD environment variable must be set");
    let default_service = env::var("NIC_SERVICE_ID").ok();
    let default_zone = env::var("NIC_ZONE").ok();

    // Create the DnsApi client. The last five parameters are:
    //   token           — a previously saved Token (None means we'll fetch one below)
    //   offline         — offline access duration in seconds (None = default)
    //   scope           — OAuth2 scope override (None = default)
    //   default_service — NIC_SERVICE_ID to use as fallback for DNS operations
    //   default_zone    — zone name to use as fallback for DNS operations
    //
    // Note: `get_token()` and `refresh_token()` take `&self` (not `&mut self`),
    // so the api binding does not need to be mutable for auth operations.
    let api = DnsApi::new(app_login, app_password, None, None, None, default_service, default_zone);

    // -------------------------------------------------------------------------
    // AUTO-RETRY ON TOKEN EXPIRY
    // -------------------------------------------------------------------------
    // All API calls automatically detect a token expiry response (error 4097),
    // refresh the token via the stored refresh_token, and transparently retry
    // the original request. No manual handling is needed.
    //
    // You can also access and store the current token for later reuse:
    //
    //   let saved_token = api.token().await;          // get current Token
    //   api.set_token(some_token).await;              // restore a saved Token
    //   let new_token = api.refresh_token("...").await?; // manual refresh

    // Obtain an access token using the user's credentials.
    println!("=== Authenticating ===");
    let token = match api.get_token(username, password).await {
        Ok(t) => {
            println!("Token obtained successfully.");
            t
        }
        Err(e) => {
            eprintln!("Failed to authenticate: {}", e);
            return;
        }
    };

    // The Token struct can be serialized and stored for later use. On
    // subsequent runs you can pass it directly to DnsApi::new() and call
    // refresh_token() to avoid asking for the password again.
    println!("Access token (truncated): {}...", &token.access_token[..20.min(token.access_token.len())]);

    // -------------------------------------------------------------------------
    // 2. SETTING DEFAULTS
    // -------------------------------------------------------------------------
    // DnsApi uses default_service and default_zone as fallbacks when None is
    // passed to zone/record methods. They can be provided at construction time
    // via the NIC_SERVICE_ID and NIC_ZONE environment variables (read above),
    // or set/overridden after construction using the setter methods:
    //
    // api.set_default_service("MY_SERVICE");   // requires &mut api
    // api.set_default_zone("example.com");     // requires &mut api
    //
    // For this example the values from environment variables are used when set,
    // otherwise explicit values are passed to each method call below.

    // -------------------------------------------------------------------------
    // 3. LISTING SERVICES
    // -------------------------------------------------------------------------
    println!("\n=== Services ===");
    match api.services().await {
        Ok(services) => {
            if services.is_empty() {
                println!("No services found on this account.");
            }
            for svc in &services {
                println!(
                    "  Service: {name}  tariff={tariff}  \
                     domains={used}/{limit}  enabled={enable}  payer={payer}",
                    name    = svc.name,
                    tariff  = svc.tariff,
                    used    = svc.domains_num,
                    limit   = svc.domains_limit,
                    enable  = svc.enable,
                    payer   = svc.payer,
                );
                // rr_limit / rr_num are optional (not all tariffs expose them)
                if let (Some(rr_num), Some(rr_limit)) = (svc.rr_num, svc.rr_limit) {
                    println!("    Resource records: {rr_num}/{rr_limit}");
                }
            }
        }
        Err(e) => eprintln!("Error fetching services: {}", e),
    }

    // -------------------------------------------------------------------------
    // 4. LISTING ZONES
    // -------------------------------------------------------------------------
    // Passing None as the service filter returns zones for the default service.
    // To limit to a specific service pass Some("MY_SERVICE").
    println!("\n=== Zones ===");
    match api.zones(None).await {
        Ok(zones) => {
            if zones.is_empty() {
                println!("No zones found.");
            }
            for zone in &zones {
                println!(
                    "  Zone: {name}  idn={idn}  service={svc}  \
                     has_changes={chg}  enabled={en}",
                    name = zone.name,
                    idn  = zone.idn_name,
                    svc  = zone.service,
                    chg  = zone.has_changes,
                    en   = zone.enable,
                );
            }
        }
        Err(e) => eprintln!("Error fetching zones: {}", e),
    }

    // -------------------------------------------------------------------------
    // 4b. LIST ALL ZONES ACROSS ALL SERVICES
    // -------------------------------------------------------------------------
    // zones_all() calls GET /dns-master/zones (no service filter required).
    println!("\n=== All zones (across all services) ===");
    match api.zones_all().await {
        Ok(zones) => {
            println!("Total zones across all services: {}", zones.len());
            for zone in &zones {
                println!("  {} (service: {})", zone.name, zone.service);
            }
        }
        Err(e) => eprintln!("Error fetching all zones: {}", e),
    }

    // -------------------------------------------------------------------------
    // 5. LISTING RECORDS
    // -------------------------------------------------------------------------
    // Both the service and zone filters default to api.default_service /
    // api.default_zone when None is passed. Here both are None so all records
    // visible to the account are returned (subject to server-side limits).
    println!("\n=== DNS Records ===");
    match api.records(None, None).await {
        Ok(records) => {
            if records.is_empty() {
                println!("No records found.");
            }
            for rec in &records {
                println!(
                    "  [{id:>10}]  {rtype:<6}  {name}",
                    id    = rec.id().map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                    rtype = record_type_name(rec),
                    name  = rec.name(),
                );
            }
        }
        Err(e) => eprintln!("Error fetching records: {}", e),
    }

    // -------------------------------------------------------------------------
    // 6 & 7. BUILDING RECORDS (no live API calls yet)
    // -------------------------------------------------------------------------
    // Demonstrate how to construct the most common record types.
    println!("\n=== Building record objects (no API call) ===");

    // A record with a custom TTL.
    let a_rec = ARecord::new("www", "93.184.216.34")
        .with_ttl(300)
        .expect("TTL must not be zero");
    println!("  Built A record:    name={} addr={}", a_rec.name, a_rec.a);

    // AAAA record.
    let aaaa_rec = AaaaRecord::new("www", "2606:2800:220:1:248:1893:25c8:1946")
        .with_ttl(300)
        .expect("TTL must not be zero");
    println!("  Built AAAA record: name={} addr={}", aaaa_rec.name, aaaa_rec.aaaa);

    // TXT record (e.g. for SPF or domain verification).
    let txt_rec = TxtRecord::new("@", "v=spf1 include:_spf.example.com ~all")
        .with_ttl(3600)
        .expect("TTL must not be zero");
    println!("  Built TXT record:  name={} txt={}", txt_rec.name, txt_rec.txt);

    // MX record.
    let mx_rec = MxRecord::new("@", 10, "mail.example.com")
        .with_ttl(3600)
        .expect("TTL must not be zero");
    println!(
        "  Built MX record:   name={}  preference={}  exchange={}",
        mx_rec.name, mx_rec.preference, mx_rec.exchange
    );

    // -------------------------------------------------------------------------
    // MODIFIES LIVE DNS — Add, commit, delete records, zone lifecycle, etc.
    // -------------------------------------------------------------------------
    // The block below is commented out to prevent accidental modification of
    // live DNS data. Un-comment it (and set `TARGET_SERVICE` / `TARGET_ZONE`)
    // only when you are ready to test against your real account.

    /*
    const TARGET_SERVICE: &str = "MY_SERVICE";  // e.g. "my_nic_service"
    const TARGET_ZONE:    &str = "example.com"; // zone the record belongs to

    // -------------------------------------------------------------------------
    // 6. ADDING A SINGLE RECORD  [MODIFIES LIVE DNS]
    // -------------------------------------------------------------------------
    println!("\n=== Adding A record (LIVE) ===");
    let new_a = ARecord::new("test-rust", "203.0.113.1")
        .with_ttl(120)
        .expect("TTL must not be zero");

    let added = match api
        .add_record(
            vec![DnsRecord::A(new_a)],
            Some(TARGET_SERVICE),
            Some(TARGET_ZONE),
        )
        .await
    {
        Ok(recs) => {
            println!("Record(s) staged successfully:");
            for r in &recs {
                println!("  id={:?}  name={}", r.id(), r.name());
            }
            recs
        }
        Err(e) => {
            eprintln!("Error adding record: {}", e);
            return;
        }
    };

    // -------------------------------------------------------------------------
    // 7. ADDING MULTIPLE RECORD TYPES  [MODIFIES LIVE DNS]
    // -------------------------------------------------------------------------
    println!("\n=== Adding TXT + MX records (LIVE) ===");
    let multi_records = vec![
        DnsRecord::TXT(
            TxtRecord::new("test-rust", "hello-from-nic-api-rust")
                .with_ttl(120)
                .expect("TTL must not be zero"),
        ),
        DnsRecord::MX(
            MxRecord::new("test-rust", 20, "mail2.example.com")
                .with_ttl(300)
                .expect("TTL must not be zero"),
        ),
    ];

    match api
        .add_record(multi_records, Some(TARGET_SERVICE), Some(TARGET_ZONE))
        .await
    {
        Ok(recs) => {
            println!("Staged {} additional record(s).", recs.len());
        }
        Err(e) => eprintln!("Error staging records: {}", e),
    }

    // -------------------------------------------------------------------------
    // 8. COMMITTING CHANGES  [MODIFIES LIVE DNS]
    // -------------------------------------------------------------------------
    // Changes are not published until commit() is called.  This makes the
    // staged records above live.
    println!("\n=== Committing changes (LIVE) ===");
    match api.commit(Some(TARGET_SERVICE), Some(TARGET_ZONE)).await {
        Ok(()) => println!("Changes committed successfully."),
        Err(e) => {
            eprintln!("Commit failed: {}", e);
            return;
        }
    }

    // -------------------------------------------------------------------------
    // 9. DELETING A RECORD  [MODIFIES LIVE DNS]
    // -------------------------------------------------------------------------
    // Use the id() from the previously added record to delete it.
    if let Some(record_id) = added.first().and_then(|r| r.id()) {
        println!("\n=== Deleting record id={} (LIVE) ===", record_id);
        match api
            .delete_record(record_id, Some(TARGET_SERVICE), Some(TARGET_ZONE))
            .await
        {
            Ok(()) => println!("Record {} deleted (staged). Remember to commit again.", record_id),
            Err(e) => eprintln!("Delete failed: {}", e),
        }

        // Commit the deletion
        let _ = api.commit(Some(TARGET_SERVICE), Some(TARGET_ZONE)).await;
    }

    // -------------------------------------------------------------------------
    // ZONE LIFECYCLE  [MODIFIES LIVE DNS]
    // -------------------------------------------------------------------------

    // Create a new zone in a service
    println!("\n=== Creating zone (LIVE) ===");
    match api.create_zone("newzone.example.com", Some(TARGET_SERVICE)).await {
        Ok(zone) => println!("Zone created: {} (id={})", zone.name, zone.id),
        Err(e) => eprintln!("Error creating zone: {}", e),
    }

    // Move a zone to another service
    println!("\n=== Moving zone (LIVE) ===");
    match api
        .move_zone(Some(TARGET_ZONE), "TARGET_SERVICE_2", Some(TARGET_SERVICE))
        .await
    {
        Ok(()) => println!("Zone moved successfully."),
        Err(e) => eprintln!("Error moving zone: {}", e),
    }

    // Delete a zone from a service
    println!("\n=== Deleting zone (LIVE) ===");
    match api.delete_zone("newzone.example.com", Some(TARGET_SERVICE)).await {
        Ok(()) => println!("Zone deleted successfully."),
        Err(e) => eprintln!("Error deleting zone: {}", e),
    }

    // -------------------------------------------------------------------------
    // ZONE FILE IMPORT / EXPORT  [MODIFIES LIVE DNS for import]
    // -------------------------------------------------------------------------

    // Export zone in BIND format
    println!("\n=== Exporting zone (LIVE) ===");
    match api.zone_export(Some(TARGET_SERVICE), Some(TARGET_ZONE)).await {
        Ok(bind_text) => {
            println!("Zone exported ({} bytes):", bind_text.len());
            // Print first 200 chars as a preview
            println!("{}", &bind_text[..200.min(bind_text.len())]);
        }
        Err(e) => eprintln!("Error exporting zone: {}", e),
    }

    // Import zone from BIND-format text (replaces existing records)
    let bind_zone = "\
$ORIGIN example.com.\n\
@ 3600 IN SOA ns1.example.com. admin.example.com. 2024010101 3600 900 604800 300\n\
@ 3600 IN NS ns1.example.com.\n\
www 300 IN A 93.184.216.34\n\
";
    println!("\n=== Importing zone (LIVE) ===");
    match api
        .zone_import(bind_zone, Some(TARGET_SERVICE), Some(TARGET_ZONE))
        .await
    {
        Ok(()) => println!("Zone imported successfully."),
        Err(e) => eprintln!("Error importing zone: {}", e),
    }

    // -------------------------------------------------------------------------
    // ROLLBACK  [MODIFIES LIVE DNS]
    // -------------------------------------------------------------------------
    // Discard all uncommitted (staged) changes for a zone.
    println!("\n=== Rollback uncommitted changes (LIVE) ===");
    match api.rollback(Some(TARGET_SERVICE), Some(TARGET_ZONE)).await {
        Ok(()) => println!("Rollback successful — staged changes discarded."),
        Err(e) => eprintln!("Error during rollback: {}", e),
    }

    // -------------------------------------------------------------------------
    // DEFAULT TTL MANAGEMENT
    // -------------------------------------------------------------------------

    // Read the current default TTL from the zone's SOA record
    println!("\n=== Default TTL ===");
    match api.get_default_ttl(Some(TARGET_SERVICE), Some(TARGET_ZONE)).await {
        Ok(ttl) => println!("Current default TTL: {} seconds", ttl),
        Err(e) => eprintln!("Error getting TTL: {}", e),
    }

    // Set a new default TTL  [MODIFIES LIVE DNS]
    match api
        .set_default_ttl(1800, Some(TARGET_SERVICE), Some(TARGET_ZONE))
        .await
    {
        Ok(()) => println!("Default TTL set to 1800 seconds."),
        Err(e) => eprintln!("Error setting TTL: {}", e),
    }

    // -------------------------------------------------------------------------
    // ZONE REVISION HISTORY
    // -------------------------------------------------------------------------
    println!("\n=== Zone revision history ===");
    match api.zone_revisions(Some(TARGET_SERVICE), Some(TARGET_ZONE)).await {
        Ok(revisions) => {
            println!("Found {} revision(s):", revisions.len());
            for rev in &revisions {
                println!(
                    "  revision #{:>4}  date={}  ip={}",
                    rev.number, rev.date, rev.ip
                );
            }
        }
        Err(e) => eprintln!("Error fetching revisions: {}", e),
    }

    // -------------------------------------------------------------------------
    // AXFR (ZONE TRANSFER) IP MANAGEMENT
    // -------------------------------------------------------------------------
    // Manage the list of IP addresses that are allowed to perform zone transfers.

    println!("\n=== AXFR IPs ===");
    match api.get_axfr_ips(Some(TARGET_SERVICE), Some(TARGET_ZONE)).await {
        Ok(ips) => println!("Allowed AXFR IPs: {:?}", ips),
        Err(e) => eprintln!("Error fetching AXFR IPs: {}", e),
    }

    // Set new AXFR IPs  [MODIFIES LIVE DNS]
    match api
        .set_axfr_ips(&["192.0.2.1", "198.51.100.2"], Some(TARGET_SERVICE), Some(TARGET_ZONE))
        .await
    {
        Ok(()) => println!("AXFR IPs updated."),
        Err(e) => eprintln!("Error setting AXFR IPs: {}", e),
    }

    // -------------------------------------------------------------------------
    // SECONDARY DNS MASTER SERVERS
    // -------------------------------------------------------------------------
    // Manage the list of master DNS servers for secondary (slave) zones.

    println!("\n=== Master servers ===");
    match api.get_masters(Some(TARGET_SERVICE), Some(TARGET_ZONE)).await {
        Ok(masters) => println!("Master servers: {:?}", masters),
        Err(e) => eprintln!("Error fetching masters: {}", e),
    }

    // Set new master servers  [MODIFIES LIVE DNS]
    match api
        .set_masters(&["192.0.2.10", "198.51.100.20"], Some(TARGET_SERVICE), Some(TARGET_ZONE))
        .await
    {
        Ok(()) => println!("Master servers updated."),
        Err(e) => eprintln!("Error setting masters: {}", e),
    }
    */

    // -------------------------------------------------------------------------
    // 10. ERROR HANDLING
    // -------------------------------------------------------------------------
    // Show how to branch on the different DnsApiError variants.
    println!("\n=== Error handling example ===");
    // Force an error by requesting a non-existent service name.
    match api.zones(Some("__nonexistent_service__")).await {
        Ok(_) => println!("(unexpected success)"),
        Err(e) => match e {
            DnsApiError::ServiceNotFound(name) => {
                println!("Service not found (expected): {}", name);
            }
            DnsApiError::ZoneNotFound(name) => {
                println!("Zone not found: {}", name);
            }
            DnsApiError::ExpiredToken => {
                // Token has expired — the library retries automatically, but if
                // manual handling is needed you can refresh here.
                println!("Token expired — refreshing…");
                // let new_token = api.refresh_token(&token.refresh_token).await?;
            }
            DnsApiError::InvalidRecord(msg) => {
                println!("Invalid record: {}", msg);
            }
            DnsApiError::HttpError(e) => {
                println!("HTTP error: {}", e);
            }
            DnsApiError::XmlError(msg) => {
                println!("XML parse error: {}", msg);
            }
            DnsApiError::OAuth2Error(msg) => {
                println!("OAuth2 error: {}", msg);
            }
            DnsApiError::ApiError(msg) => {
                println!("API error: {}", msg);
            }
            DnsApiError::InvalidTtl => {
                println!("Invalid TTL (must not be zero).");
            }
            DnsApiError::InvalidRecordId => {
                println!("Invalid record id.");
            }
            DnsApiError::ZoneAlreadyExists(name) => {
                println!("Zone already exists: {}", name);
            }
            DnsApiError::InvalidDomainName(name) => {
                println!("Invalid domain name: {}", name);
            }
        },
    }

    println!("\nDone.");
}

// ---------------------------------------------------------------------------
// Helper — return a short type-name string for a DnsRecord variant.
// ---------------------------------------------------------------------------
fn record_type_name(rec: &DnsRecord) -> &'static str {
    match rec {
        DnsRecord::A(_)     => "A",
        DnsRecord::AAAA(_)  => "AAAA",
        DnsRecord::CNAME(_) => "CNAME",
        DnsRecord::MX(_)    => "MX",
        DnsRecord::NS(_)    => "NS",
        DnsRecord::TXT(_)   => "TXT",
        DnsRecord::SOA(_)   => "SOA",
        DnsRecord::SRV(_)   => "SRV",
        DnsRecord::PTR(_)   => "PTR",
        DnsRecord::DNAME(_) => "DNAME",
        DnsRecord::HINFO(_) => "HINFO",
        DnsRecord::NAPTR(_) => "NAPTR",
        DnsRecord::RP(_)    => "RP",
    }
}
