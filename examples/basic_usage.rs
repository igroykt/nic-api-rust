use nic_api_rust::{DnsApi, models::ARecord};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("NIC.RU DNS API - Basic Usage Example\n");

    // You need to obtain OAuth application credentials from:
    // https://www.nic.ru/manager/oauth.cgi?step=oauth.app_register
    let app_login = std::env::var("NIC_APP_LOGIN")
        .expect("NIC_APP_LOGIN environment variable not set");
    let app_password = std::env::var("NIC_APP_PASSWORD")
        .expect("NIC_APP_PASSWORD environment variable not set");

    // Your NIC.RU account credentials
    let username = std::env::var("NIC_USERNAME")
        .expect("NIC_USERNAME environment variable not set");
    let password = std::env::var("NIC_PASSWORD")
        .expect("NIC_PASSWORD environment variable not set");

    // Initialize API client
    let mut api = DnsApi::new(
        app_login,
        app_password,
        None,  // No existing token
        None,  // Use default offline duration (3600 seconds)
        None,  // Use default scope
    );

    println!("🔐 Authenticating...");
    
    // Get OAuth token
    let token = api.get_token(username, password).await?;
    println!("✅ Authentication successful!");
    println!("   Access token: {}...", &token.access_token[..20]);
    if let Some(expires_in) = token.expires_in {
        println!("   Expires in: {} seconds", expires_in);
    }
    println!();

    // Get available services
    println!("📋 Fetching available services...");
    let services = api.services().await?;
    
    if services.is_empty() {
        println!("   No services found.");
    } else {
        println!("   Found {} service(s):", services.len());
        for service in &services {
            println!("   - {} (domains: {}/{})", 
                service.name, 
                service.domains_num, 
                service.domains_limit
            );
        }
    }
    println!();

    // Example: Set default service and zone
    if let Some(first_service) = services.first() {
        api.default_service = Some(first_service.name.clone());
        println!("🔧 Set default service to: {}", first_service.name);
        
        // You can now work with zones and records
        // Note: These operations require a valid service and zone
        
        // Example: Get zones (commented out - uncomment when you have valid data)
        // let zones = api.zones(None).await?;
        // println!("   Zones: {:?}", zones);

        // Example: Create an A record (commented out - uncomment when ready to test)
        // api.default_zone = Some("example.com".to_string());
        // let record = ARecord::new("www", "192.168.1.1")
        //     .with_ttl(3600)?;
        // let created = api.add_record(vec![DnsRecord::A(record)], None, None).await?;
        // println!("   Created record: {:?}", created);
        
        // Don't forget to commit changes!
        // api.commit(None, None).await?;
    }

    println!("\n✨ Example completed successfully!");
    
    Ok(())
}
